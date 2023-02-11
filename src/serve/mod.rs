use crate::{content, errors::Error, templates, CommandRun};
use clap::{Args, ValueEnum};

use log::{debug, error, info};
use simplelog::{self, TermLogger};
use std::{
    fs,
    io::{self, BufReader, Cursor},
    path::{Path, PathBuf},
    process::{self, exit},
    time::Duration,
};
use wruster::{
    http::{
        headers::{Header, Headers},
        Body, HttpMethod, Request, Response, StatusCode,
    },
    router::{HttpHandler, Router},
    Server, Timeouts,
};

mod middleware;

const POST_SUBDIR: &str = "posts";
const ASSETS_SUBDIR: &str = "assets";
const ASSETS_ROUTE: &str = "/assets";
const POSTS_ROUTE: &str = "/posts";
const POST_ASSETS_ROUTE: &str = "/posts/post_assets";

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum LogLevel {
    Off,
    Error,
    Info,
    Debug,
}

impl From<LogLevel> for simplelog::LevelFilter {
    fn from(val: LogLevel) -> Self {
        match val {
            LogLevel::Off => simplelog::LevelFilter::Off,
            LogLevel::Error => simplelog::LevelFilter::Error,
            LogLevel::Info => simplelog::LevelFilter::Info,
            LogLevel::Debug => simplelog::LevelFilter::Debug,
        }
    }
}

#[derive(Args, Debug)]
pub(crate) struct ServeCommand {
    /// Path to a directory containing the blog templates.
    templates: String,
    /// Path to a directory containing the blog contents.
    content: String,
    /// Address to listen to, for example: localhost:8080
    address: String,
    /// Log level: off, error, info, debug
    #[arg(short, long, value_enum, default_value_t = LogLevel::Info)]
    level: LogLevel,
}

impl CommandRun for ServeCommand {
    fn run(&self) {
        TermLogger::init(
            self.level.into(),
            simplelog::Config::default(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        )
        .unwrap_or_else(|error| {
            eprintln!("unexpected error initializing the log level: {}", error);
            exit(1);
        });

        // Load the templates of rhe blog.
        let blog_templates =
            templates::Blog::read_from_dir(&self.templates).unwrap_or_else(|err| {
                let err = Error::Undefined(format!("invalid templates path: {}", err));
                err.fatal();
                exit(1);
            });

        let templates_assets_path = Path::new(&self.templates)
            .join(ASSETS_SUBDIR)
            .canonicalize()
            .unwrap_or_else(|err| {
                let err =
                    Error::Undefined(format!("error reading the templates assets dir: {}", err));
                err.fatal();
                exit(1);
            });

        let content_path = PathBuf::from(&self.content)
            .canonicalize()
            .unwrap_or_else(|err| {
                let err = Error::Undefined(format!("invalid content path: {}", err));
                err.fatal();
                exit(1);
            });

        // Build the router.
        //let router = build_router(templates_assets_path, content_path, blog_templates);
        let router = build_simple_router(templates_assets_path, content_path, blog_templates);
        // Start the web server.
        let timeouts = Timeouts {
            write_response_timeout: Duration::from_secs(5),
            read_request_timeout: Duration::from_secs(5),
        };
        let mut server = Server::from_timeouts(timeouts);
        server.run(&self.address, router).unwrap_or_else(|err| {
            error!("running wruster {}", err.to_string());
            process::exit(1);
        });
        server.wait().unwrap_or_else(|err| {
            error!("error running wruster {}", err.to_string());
            process::exit(1);
        });
        process::exit(0);
    }
}

fn build_simple_router(
    template_assets_dir: PathBuf,
    content_dir: PathBuf,
    blog_templates: templates::Blog,
) -> Router {
    let router = Router::new();
    // Handler for the static assets of the templates.
    debug!(
        "serving template assets from dir: {}",
        template_assets_dir.to_string_lossy()
    );
    let (main_template, post_template) = blog_templates.parts();

    // /
    // index
    let main_handler_content_dir = content_dir.clone();
    let main_handler = move |request: &mut Request| -> Response {
        serve_main_page(main_handler_content_dir.clone(), request, &main_template)
    };
    let main_handler: HttpHandler = Box::new(main_handler);
    router.add("/", HttpMethod::GET, main_handler);

    // assets/.+
    let assets_handler = move |request: &mut Request| -> Response {
        serve_static(
            ASSETS_ROUTE.into(),
            template_assets_dir.clone(),
            request,
            Some(vec!["md"]),
        )
    };
    let assets_handler: HttpHandler = Box::new(assets_handler);
    router.add(ASSETS_ROUTE, HttpMethod::GET, assets_handler);

    // posts/post_article
    let post_handler_content_dir = content_dir.clone();
    let posts_handler = move |request: &mut Request| -> Response {
        serve_post(post_handler_content_dir.clone(), request, &post_template)
    };
    let posts_handler: HttpHandler = middleware::log(Box::new(posts_handler));
    router.add(POSTS_ROUTE, HttpMethod::GET, posts_handler);

    // post assets route /posts/post_assets
    let post_asssets_dir = content_dir.join(POST_SUBDIR);
    let posts_assets_handler = move |request: &mut Request| -> Response {
        serve_static(
            POST_ASSETS_ROUTE.into(),
            post_asssets_dir.clone(),
            request,
            Some(vec!["md"]),
        )
    };
    let posts_assets_handler: HttpHandler = Box::new(posts_assets_handler);
    router.add(POST_ASSETS_ROUTE, HttpMethod::GET, posts_assets_handler);

    router
}

pub fn serve_post(
    content_dir: PathBuf,
    request: &Request,
    templates: &templates::Post,
) -> Response {
    let mut uri = PathBuf::from(request.uri.as_str());
    if uri.extension().unwrap_or_default() == "md" {
        debug!(
            "handle_blog_request: discarding request to .md file: {}",
            uri.display(),
        );
        return Response::from_status(StatusCode::NotFound);
    }
    debug!("serving content, raw request uri: {}", uri.display());
    // Remove the route from the path.
    uri = match uri.strip_prefix::<PathBuf>(POSTS_ROUTE.into()) {
        Ok(uri) => uri.to_path_buf(),
        Err(err) => {
            debug!("serving content, bad request, error: {}", err.to_string());
            return Response::from_status(StatusCode::BadRequest);
        }
    };
    debug!(
        "serving content for a post, uri: {}, file name: {}",
        uri.display(),
        uri.file_name().unwrap_or_default().to_string_lossy(),
    );
    let post_file = uri
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    match generate_post_content(templates, &content_dir, post_file) {
        Ok(content) => {
            let content_len = content.len() as u64;
            let content = Cursor::new(content);
            Response::from_content(content, content_len, mime::TEXT_HTML)
        }
        Err(err) => {
            error!("serving content error generating post content: {}", err);
            Response::from_status(StatusCode::InternalServerError)
        }
    }
}

pub fn serve_main_page(
    content_dir: PathBuf,
    request: &Request,
    templates: &templates::Main,
) -> Response {
    info!("serving content, raw request uri: {}", request.uri);
    let uri = request.uri.as_str();
    match uri {
        "/" | "" => match generate_main_page_content(templates, &content_dir) {
            Ok(content) => {
                let content_len = content.len() as u64;
                let content = Cursor::new(content);
                Response::from_content(content, content_len, mime::TEXT_HTML)
            }
            Err(err) => {
                error!(
                    "serving content error generating main page content: {}",
                    err
                );
                Response::from_status(StatusCode::InternalServerError)
            }
        },
        _ => Response::from_status(StatusCode::NotFound),
    }
}

fn generate_post_content(
    templates: &templates::Post,
    post_content_dir: &PathBuf,
    post_file: &str,
) -> Result<String, Error> {
    let post_file_path = Path::new(post_content_dir)
        .join(POST_SUBDIR)
        .join(post_file);
    let post_file_path = match post_file_path.to_str() {
        Some(file_path) => file_path,
        None => return Err(Error::Undefined("invalid path".into())),
    };
    let post_file_path = format!("{}.md", post_file_path);
    debug!("generating post content from file: {}", post_file_path);
    let post = content::read_post_file(&post_file_path)?;
    let post_model = templates::PostTemplateModel {
        author: post.author,
        title: post.title,
        root_page: "/".into(),
        content: post.content,
        date: templates::DateTime(post.date.0),
        favorite: post.favorite,
        file_name: post_file.into(),
        summary: post.summary,
        tags: post
            .tags
            .iter()
            .map(|tag| templates::Tag(tag.0.clone()))
            .collect(),
        year: post.year,
    };
    Ok(templates.render(&post_model))
}

fn generate_main_page_content(
    templates: &templates::Main,
    content_dir: &Path,
) -> Result<String, Error> {
    let posts_dir = content_dir.join(POST_SUBDIR);
    let posts_dir = posts_dir.to_string_lossy();

    let content_dir = content_dir.to_string_lossy();

    let blog_content = content::read_blog_file(&content_dir)?;
    let posts_metadata = content::read_posts_metadata(&posts_dir)?;

    let posts_template_models = posts_metadata
        .into_iter()
        .map(|metadata| {
            let mut file_name = metadata.file_name.replace(".md", "");
            file_name = format!("{}/{}", POSTS_ROUTE, file_name);
            templates::PostTemplateModel {
                author: metadata.author,
                title: metadata.title,
                content: "".into(),
                date: templates::DateTime(metadata.date.0),
                file_name,
                root_page: "/".into(),
                summary: metadata.summary,
                tags: metadata
                    .tags
                    .iter()
                    .map(|tag| templates::Tag(tag.0.clone()))
                    .collect(),
                favorite: false,
                year: "".into(),
            }
        })
        .collect();

    let main_template_model = templates::MainTemplateModel {
        author: blog_content.author,
        title: blog_content.title,
        home_content: blog_content.home_content,
        twitter: blog_content.twitter,
        year: blog_content.year,
        posts: posts_template_models,
    };
    Ok(templates.render(&main_template_model))
}

pub fn serve_static(
    route: String,
    base_dir: PathBuf,
    request: &Request,
    exclude_extensions: Option<Vec<&str>>,
) -> Response {
    debug!(
        "serving static from base dir: {}",
        base_dir.to_str().unwrap()
    );
    let mut uri = PathBuf::from(request.uri.as_str());
    if let Some(exclude) = exclude_extensions {
        if uri.has_any_extension(exclude) {
            return Response::from_status(StatusCode::NotFound);
        }
    }
    // Remove the route from the path.
    if uri.starts_with(route.as_str()) {
        uri = match uri.strip_prefix::<PathBuf>(route.into()) {
            Ok(uri) => uri.to_path_buf(),
            Err(err) => {
                debug!("serving static bad request, error: {}", err.to_string());
                return Response::from_status(StatusCode::BadRequest);
            }
        }
    }
    // Do not allow serving the root of the route.
    let mut uri = uri.to_str().unwrap();
    if uri.starts_with('/') {
        if uri.len() < 2 {
            return Response::from_status(StatusCode::NotFound);
        }
        uri = &uri[1..]
    }
    // Append the path minus the route to the base directory.
    let mut path = base_dir;
    path.push(uri);
    debug!(
        "serving static resource from path: {}",
        path.to_str().unwrap()
    );
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => {
            if let io::ErrorKind::NotFound = err.kind() {
                return Response::from_status(StatusCode::NotFound);
            }
            return Response::from_status(StatusCode::InternalServerError);
        }
    };

    let content = match fs::File::open(&path) {
        Ok(content) => content,
        Err(err) => {
            if let io::ErrorKind::NotFound = err.kind() {
                return Response::from_status(StatusCode::NotFound);
            }
            return Response::from_status(StatusCode::InternalServerError);
        }
    };
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();
    let mut headers = Headers::new();
    let content = Box::new(BufReader::new(content));
    headers.add(Header {
        name: String::from("Content-Length"),
        value: metadata.len().to_string(),
    });
    headers.add(Header {
        name: String::from("Content-Type"),
        value: mime_type.to_string(),
    });
    let body = Body::new(Some(mime_type), metadata.len(), content);
    Response {
        status: StatusCode::OK,
        headers,
        body: Some(body),
    }
}

trait HasAnyExtension
where
    Self: std::marker::Sized,
{
    fn has_any_extension(&self, extensions: Vec<&str>) -> bool;
}

impl HasAnyExtension for PathBuf {
    fn has_any_extension(&self, extensions: Vec<&str>) -> bool {
        let extension = match self.extension() {
            Some(extension) => extension.to_str().unwrap_or(""),
            None => return false,
        };
        extensions
            .into_iter()
            .any(|current_extension| current_extension == extension)
    }
}
