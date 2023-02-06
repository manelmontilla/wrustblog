use std::fmt::Display;

pub enum Error {
    Undefined(String),
    NoFrontMatter(String),
    NoBlogTemplateFound,
    NoPostsTemplateFound,
}

impl Error {
    pub fn fatal(self) {
        eprintln!("Application error: {self}");
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Undefined(message) => write!(f, "{}", message),
            Error::NoFrontMatter(file) => write!(f, "no front matter in {}", file),
            Error::NoBlogTemplateFound => write!(f, "no main template file found"),
            Error::NoPostsTemplateFound => write!(f, "no posts template file found"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Undefined(error.to_string())
    }
}

impl From<ramhorns::Error> for Error {
    fn from(error: ramhorns::Error) -> Self {
        Error::Undefined(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Undefined(error.to_string())
    }
}
