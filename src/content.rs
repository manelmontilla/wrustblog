use std::path::{self, PathBuf};

use chrono::{self, Datelike, Utc};
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{html, CowStr, Event, Options, Parser as MDParser};
use serde::{self, Deserialize, Deserializer};

use crate::errors::Error;

#[derive(Deserialize, Debug)]
pub struct Blog {
    pub title: String,
    pub twitter: String,
    pub author: String,
    pub year: u16,
    #[serde(default)]
    pub home_content: String,
    #[serde(default)]
    pub posts: Vec<Post>,
    #[serde(default)]
    pub post_assets: Vec<PathBuf>,
}

impl Blog {
    pub fn read_from(dir: &str) -> Result<Blog, Error> {
        let post_items = read_post_files(dir)?;
        let mut posts: Vec<Post> = Vec::new();
        let mut post_assets: Vec<PathBuf> = Vec::new();
        for item in post_items {
            match item {
                PostItem::Content(post) => posts.push(post),
                PostItem::Asset(asset) => post_assets.push(asset),
            };
        }
        posts.sort_by(|a, b| b.date.0.cmp(&a.date.0));
        let mut blog = read_blog_file(dir)?;
        blog.posts = posts;
        blog.post_assets = post_assets;
        Ok(blog)
    }
}

pub(crate) fn read_blog_file(dir: &str) -> Result<Blog, Error> {
    let blog_file = path::Path::new(&dir).join("blog.md");
    let blog_contents = std::fs::read_to_string(blog_file.clone())?;
    let (content, front_matter) = split_content(&blog_contents);
    if front_matter.is_empty() {
        return Err(Error::NoFrontMatter(blog_file.to_string_lossy().into()));
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = MDParser::new_ext(&content, options);
    let parser = process_markdown_images(parser);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    let matter = Matter::<YAML>::new();
    let result = matter.parse(&front_matter);
    let data = match result.data {
        Some(data) => data,
        None => {
            return Err(Error::NoFrontMatter(blog_file.to_string_lossy().into()));
        }
    };
    let mut blog: Blog = data.deserialize()?;
    blog.home_content = html_output;
    Ok(blog)
}

enum PostItem {
    Content(Post),
    Asset(PathBuf),
}

#[derive(Deserialize, Debug)]
pub struct Post {
    pub title: String,
    #[serde(deserialize_with = "parse_date_time")]
    pub date: DateTime,
    pub tags: Vec<Tag>,
    pub summary: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub file_name: String,
    pub author: String,
    #[serde(default)]
    pub year: String,
}

#[derive(Deserialize, Debug)]
pub struct PostMetadata {
    pub title: String,
    #[serde(deserialize_with = "parse_date_time")]
    pub date: DateTime,
    pub tags: Vec<Tag>,
    pub summary: String,
    pub author: String,
    #[serde(default)]
    pub file_name: String,
}

#[derive(Deserialize, Debug)]
pub struct Tag(pub String);

fn parse_date_time<'a, D: Deserializer<'a>>(d: D) -> Result<DateTime, D::Error> {
    let date: String = Deserialize::deserialize(d)?;
    // Expected format example: 2015-09-05 23:56:04.
    let datetime = match chrono::NaiveDateTime::parse_from_str(&date, "%Y-%m-%d %H:%M") {
        Ok(d) => match d.and_local_timezone(chrono::Utc) {
            chrono::LocalResult::None => {
                panic!("error deserialzing date time");
            }
            chrono::LocalResult::Single(d) => d,
            chrono::LocalResult::Ambiguous(_, _) => {
                panic!("error deserializing date time ambiguous")
            }
        },
        Err(err) => {
            return Err(serde::de::Error::custom(err.to_string()));
        }
    };
    Ok(DateTime(datetime))
}

#[derive(Debug)]
pub struct DateTime(pub chrono::DateTime<Utc>);

fn read_post_files(content_path: &str) -> Result<Vec<PostItem>, Error> {
    let posts_dir_path = path::Path::new(&content_path).join("posts");
    let mut post_items: Vec<PostItem> = Vec::new();
    for entry in std::fs::read_dir(posts_dir_path).map_err(Error::from)? {
        let entry = entry.map_err(Error::from)?;
        let entry_type = entry.file_type().map_err(Error::from)?;
        if !entry_type.is_file() {
            continue;
        }
        if let Some(ext) = entry.path().extension() {
            let ext = ext.to_str().unwrap_or("");
            if ext != "md" {
                // We consider any file with an extension different to "md"
                // an asset, so we just push the path.
                let asset_path = entry.path();
                post_items.push(PostItem::Asset(asset_path));
                continue;
            }
            let post_path = entry.path();
            let post_path = post_path.to_str().unwrap_or("");
            let post = read_post_file(post_path)?;
            post_items.push(PostItem::Content(post));
        }
    }

    Ok(post_items)
}

pub(crate) fn read_posts_metadata(posts_path: &str) -> Result<Vec<PostMetadata>, Error> {
    let mut posts_metadata: Vec<PostMetadata> = Vec::new();
    for entry in std::fs::read_dir(posts_path).map_err(Error::from)? {
        let entry = entry.map_err(Error::from)?;
        let entry_type = entry.file_type().map_err(Error::from)?;
        if !entry_type.is_file() {
            continue;
        }
        if let Some(ext) = entry.path().extension() {
            let ext = ext.to_str().unwrap_or("");
            if ext != "md" {
                continue;
            }
            let post_path = entry.path();
            let post_path = post_path.to_str().unwrap_or("");
            let metadata = read_post_metadata(post_path)?;
            posts_metadata.push(metadata);
        }
    }
    Ok(posts_metadata)
}

pub(crate) fn read_post_metadata(post_path: &str) -> Result<PostMetadata, Error> {
    let blog_contents = std::fs::read_to_string(post_path)?;
    let (_, front_matter) = split_content(&blog_contents);
    if front_matter.is_empty() {
        return Err(Error::NoFrontMatter(post_path.into()));
    }

    let matter = Matter::<YAML>::new();
    let result = matter.parse(&front_matter);
    let data = match result.data {
        Some(data) => data,
        None => {
            return Err(Error::NoFrontMatter(post_path.into()));
        }
    };
    let mut metadata: PostMetadata = data.deserialize()?;
    metadata.file_name = PathBuf::from(post_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into();
    Ok(metadata)
}

pub(crate) fn read_post_file(post_path: &str) -> Result<Post, Error> {
    let blog_contents = std::fs::read_to_string(post_path)?;
    let (content, front_matter) = split_content(&blog_contents);
    if front_matter.is_empty() {
        return Err(Error::NoFrontMatter(post_path.into()));
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = MDParser::new_ext(&content, options);
    let parser = process_markdown_images(parser);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    let matter = Matter::<YAML>::new();
    let result = matter.parse(&front_matter);
    let data = match result.data {
        Some(data) => data,
        None => {
            return Err(Error::NoFrontMatter(post_path.into()));
        }
    };
    let mut post: Post = data.deserialize()?;
    post.content = html_output;
    let post_path = path::Path::new(&post_path);
    let mut post_path = path::PathBuf::from(post_path);
    post_path.set_extension("html");
    let file_name = post_path
        .file_name()
        .map_or_else(|| "".to_string(), |path| path.to_string_lossy().to_string());
    post.file_name = file_name;
    post.year = post.date.0.year().to_string();
    Ok(post)
}

fn process_markdown_images<'a>(
    parser: MDParser<'a, 'a>,
) -> Box<dyn Iterator<Item = Event<'a>> + 'a> {
    let parser = parser.map(|event| match &event {
        Event::Start(pulldown_cmark::Tag::Image(link_type, url, title)) => {
            let url = format!("post_assets/{}", url);
            let tag = pulldown_cmark::Tag::Image(*link_type, CowStr::from(url), title.clone());
            Event::Start(tag)
        }
        _ => event,
    });
    Box::new(parser)
}

fn split_content(content: &str) -> (String, String) {
    let delimiter = "---";
    let rest = match content.starts_with(delimiter) {
        true => &content[delimiter.len()..],
        false => return (String::from(content), String::from("")),
    };
    let (content, front_matter) = match rest.find(delimiter) {
        Some(end) => (
            &rest[end + delimiter.len()..],
            &content[..end + 2 * delimiter.len()],
        ),
        None => (content, ""),
    };
    (String::from(content), String::from(front_matter))
}
