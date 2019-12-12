#![allow(unused)]

pub use extract::{DefaultExtractor, Extractor};
pub use language::Language;
pub use newspaper::{Config, Newspaper, NewspaperBuilder};

pub mod article;
pub mod clean;
pub mod date;
mod error;
pub mod extract;
pub mod fulltext;
pub mod image;
pub mod language;
pub mod newspaper;
pub mod stopwords;
pub mod storage;
pub mod summarize;
pub mod video;
