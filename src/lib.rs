#![allow(unused)]

pub use extract::{DefaultExtractor, Extractor};
pub use language::Language;
pub use newspaper::{ArticleStream, Category, Config, Newspaper, NewspaperBuilder};

pub mod article;
pub mod clean;
pub mod date;
mod error;
pub mod extract;
pub mod image;
pub mod language;
pub mod newspaper;
pub mod stopwords;
pub mod summarize;
pub mod text;
pub mod video;
