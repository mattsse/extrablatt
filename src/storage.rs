use std::path::{Path, PathBuf};

use reqwest::Url;

use crate::article::Article;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ArticleStore {
    dir: PathBuf,
}

impl ArticleStore {
    pub fn new<T: AsRef<Path>>(dir: T) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    pub(crate) async fn insert(&mut self, article: &Article) {
        unimplemented!()
    }

    pub(crate) async fn update(&mut self, article: &Article) {
        unimplemented!()
    }

    pub(crate) async fn delete(&mut self, article: &Url) {
        unimplemented!()
    }

    pub(crate) async fn get(&mut self, article: &Url) {
        unimplemented!()
    }
}
