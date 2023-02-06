use chrono::{self, Utc};
use ramhorns::{self, Content};

use crate::errors::Error;

const MAIN_TEMPLATE: &str = "index.html";
const POST_TEMPLATE: &str = "post.html";

pub struct Main {
    templates: ramhorns::Ramhorns,
}

impl Main {
    pub(crate) fn read_from_dir(templates_dir: &str) -> Result<Main, Error> {
        let templates = ramhorns::Ramhorns::from_folder(templates_dir).map_err(Error::from)?;
        if templates.get(MAIN_TEMPLATE).is_none() {
            return Err(Error::NoBlogTemplateFound);
        }
        let main = Main { templates };
        Ok(main)
    }

    pub(crate) fn render(&self, model: &MainTemplateModel) -> String {
        let tpl = self.templates.get(MAIN_TEMPLATE).unwrap();
        tpl.render(model)
    }
}

pub struct Post {
    templates: ramhorns::Ramhorns,
}

impl Post {
    pub(crate) fn read_from_dir(templates_dir: &str) -> Result<Post, Error> {
        let templates = ramhorns::Ramhorns::from_folder(templates_dir).map_err(Error::from)?;
        if templates.get(POST_TEMPLATE).is_none() {
            return Err(Error::NoBlogTemplateFound);
        }
        let post = Post { templates };
        Ok(post)
    }

    pub(crate) fn render(&self, model: &PostTemplateModel) -> String {
        let tpl = self.templates.get(POST_TEMPLATE).unwrap();
        tpl.render(model)
    }
}

pub struct Blog {
    main: Main,
    post: Post,
}

impl Blog {
    pub(crate) fn read_from_dir(templates_dir: &str) -> Result<Blog, Error> {
        let main = Main::read_from_dir(templates_dir)?;
        let post = Post::read_from_dir(templates_dir)?;
        let blog = Blog { main, post };
        Ok(blog)
    }

    pub(crate) fn render_main(&self, model: &MainTemplateModel) -> String {
        self.main.render(model)
    }

    pub(crate) fn render_post(&self, model: &PostTemplateModel) -> String {
        self.post.render(model)
    }

    pub(crate) fn parts(self) -> (Main, Post) {
        (self.main, self.post)
    }
}

#[derive(Content, Debug)]
pub struct MainTemplateModel {
    pub title: String,
    pub twitter: String,
    pub home_content: String,
    pub author: String,
    pub year: u16,
    pub posts: Vec<PostTemplateModel>,
}

#[derive(Content, Debug)]
pub struct PostTemplateModel {
    pub title: String,
    #[ramhorns(callback = render_date_time)]
    pub date: DateTime,
    pub tags: Vec<Tag>,
    pub summary: String,
    pub root_page: String,
    pub content: String,
    pub favorite: bool,
    pub file_name: String,
    pub author: String,
    pub year: String,
}

fn render_date_time<E>(s: &DateTime, enc: &mut E) -> Result<(), E::Error>
where
    E: ramhorns::encoding::Encoder,
{
    let date_time = s.0.format("%Y-%m-%d %H:%M").to_string();
    enc.write_escaped(&date_time)
}

#[derive(Debug)]
pub struct DateTime(pub chrono::DateTime<Utc>);

impl Content for DateTime {}

#[derive(Content, Debug)]
pub struct Tag(pub String);
