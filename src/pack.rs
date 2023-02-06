use std::{
    fs, io,
    path::{Path, PathBuf},
    process::exit,
    str::FromStr,
};

use clap::Args;

use crate::{content, errors::Error, templates, CommandRun};

const POST_ASSETS_DIR: &str = "post_assets";
const ASSETS_DIR: &str = "assets";

#[derive(Args, Debug)]
pub(crate) struct PackCommand {
    /// Path to a directory containing the blog templates.
    templates: String,
    /// Path to a directory containing the blog contents.
    content: String,
    /// Path to a directory for the generated content files.
    output: String,
}

impl CommandRun for PackCommand {
    fn run(&self) {
        // Copy the template assets directory to the output directory.
        let templates_path = PathBuf::from(&self.templates);
        let output_path = PathBuf::from(&self.output);
        let assets_path = PathBuf::from(ASSETS_DIR);
        let dest_assets_path = output_path.join(&assets_path);
        ensure_dir_is_empty(&dest_assets_path).unwrap_or_else(|err| {
            err.fatal();
            exit(1);
        });
        copy_dir(&templates_path, &output_path, &assets_path).unwrap_or_else(|err| {
            err.fatal();
            exit(1);
        });

        // Load the templates of rhe blog.
        let blog_templates =
            templates::Blog::read_from_dir(&self.templates).unwrap_or_else(|err| {
                err.fatal();
                exit(1);
            });

        // Read the content of the blog.
        let blog_content = content::Blog::read_from(&self.content).unwrap_or_else(|err| {
            err.fatal();
            exit(1);
        });

        // Generare the template models of the blog from the content.
        let posts_template_models = blog_content
            .posts
            .iter()
            .map(|post| templates::PostTemplateModel {
                title: post.title.clone(),
                date: templates::DateTime(post.date.0),
                tags: post
                    .tags
                    .iter()
                    .map(|tag| templates::Tag(tag.0.clone()))
                    .collect(),
                summary: post.summary.clone(),
                root_page: "index.html".into(),
                content: post.content.clone(),
                favorite: post.favorite,
                file_name: post.file_name.clone(),
                author: post.author.clone(),
                year: post.year.clone(),
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

        // Render main page.
        let main_page_content = blog_templates.render_main(&main_template_model);
        let main_page_path = Path::new(&self.output);
        let main_page_path = main_page_path.join("index.html");
        fs::write(main_page_path, main_page_content)
            .map_err(Error::from)
            .unwrap_or_else(|err| {
                err.fatal();
                exit(1);
            });
        // Render blog posts.
        for template_post in main_template_model.posts {
            let post_path = Path::new(&self.output)
                .join(template_post.file_name.clone())
                .clone();
            let post_content = blog_templates.render_post(&template_post);
            fs::write(post_path, post_content)
                .map_err(Error::from)
                .unwrap_or_else(|err| {
                    err.fatal();
                    exit(1);
                });
        }

        // Copy the assets of the posts to the post assets
        // directory.
        let post_assets_path = PathBuf::from(POST_ASSETS_DIR);
        let post_assets_path = output_path.join(&post_assets_path);
        for src_asset_path in blog_content.post_assets {
            ensure_dir_is_empty(&post_assets_path).unwrap_or_else(|err| {
                err.fatal();
                exit(1);
            });
            let asset_file_name = src_asset_path.file_name().unwrap_or_default();
            let asset_file_name = asset_file_name.to_str().unwrap();
            let asset_file_name = PathBuf::from_str(asset_file_name).unwrap();
            let dest_asset_path = post_assets_path.join(asset_file_name);
            fs::copy(src_asset_path, dest_asset_path)
                .map_err(Error::from)
                .unwrap_or_else(|err| {
                    err.fatal();
                    exit(1);
                });
        }
    }
}

fn ensure_dir_is_empty(dir: &PathBuf) -> Result<(), Error> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(Error::from)?
    }
    fs::create_dir(dir).map_err(Error::from)?;
    Ok(())
}

fn copy_dir(src: &PathBuf, dest: &PathBuf, current_path: &PathBuf) -> Result<(), Error> {
    let current_full_path = src.join(current_path);
    let entries = fs::read_dir(current_full_path).map_err(Error::from)?;
    for entry in entries {
        let entry = entry.map_err(Error::from)?;
        let entry_type = entry.file_type().map_err(Error::from)?;
        // We only copy files or subdirectories.
        if entry_type.is_file() {
            let dest_full_path = dest.join(current_path).join(entry.file_name());
            fs::copy(entry.path(), dest_full_path).map_err(Error::from)?;
            continue;
        }
        if entry_type.is_dir() {
            let dest_full_path = dest.join(current_path).join(entry.file_name());
            if let Err(err) = fs::create_dir(dest_full_path) {
                if err.kind() != io::ErrorKind::AlreadyExists {
                    return Err(Error::from(err));
                }
            };
            let current_dir_path = current_path.join(entry.file_name());
            copy_dir(src, dest, &current_dir_path)?
        }
    }
    Ok(())
}
