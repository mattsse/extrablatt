/// Reexported to implement custom extractors.
pub use select;

pub use article::{Article, PureArticle};
pub use extrablatt::{ArticleStream, Category, Config, Extrablatt, ExtrablattBuilder};
pub use extract::{DefaultExtractor, Extractor};
pub use language::Language;

pub mod article;
pub mod clean;
pub mod date;
mod error;
pub mod extrablatt;
pub mod extract;
pub mod image;
pub mod language;
pub mod stopwords;
pub mod text;
pub mod video;
