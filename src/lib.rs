//! Extrablatt: Customizable article scraping & curation.
//!
//! A library to gather and scrape news articles. This functionality is also
//! provided as a [CLI utility](//github.com/mattsse/extrablatt). Extrablatt
//! supports targeted article scraping and non-targeted gathering of articles
//! either limited by category or unrestricted for an entire website. Extrablatt
//! allows for customized extraction of the textual content of an article by
//! means of the [`crate::extract::Extractor`] trait.
//!
//!
//! # Examples
//!
//! Extract all content from a single news article
//!
//! ```no_run
//!  extrablatt::Article::get("http://example.com/interesting-article.html");
//! ```
//!
//! A [`crate::Category`] represents a broader collection of articles, like Sports or Politics. Categores usually have their own designated page, e.g. `https://some-news.com/sports`. By targeting specific [`crate::Category`], Extrablatt tries to identify all news articles on the categorie's page and then scrape them afterwards.
//!
//! ```no_run
//! use futures::stream::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut stream = extrablatt::Category::new("https://some-news.com/sports".parse().unwrap())
//!     .into_stream()
//!     .await?;
//! while let Some(article) = stream.next().await {
//!     //...
//! }
//! #     Ok(())
//! # }
//! ```
//!
//! The streaming of articles is made possible by [`crate::ArticleStream`],
//! which lets you turn any website into [`futures::stream::Stream`] fo
//! articles.
//!
//!
//! [`crate::Extrablatt`] is essentially a cache of articles of an entire
//! news site. It first tries to identify all categories from the main page and
//! then download and and scrape every article. Failed downloading attempts can
//! easily be repeated.
//!
//! ```no_run
//! use futures::stream::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut site = extrablatt::Extrablatt::builder("https://some-news.com/")?.build().await?;
//! site.download_all_remaining_categories().await;
//! for(url, content) in site.download_articles().await.successes() {
//!     // ...
//! }
//! #   Ok(())
//! # }
//! ```
//!
//! However [`crate::Extrablatt`] can also be consumed and turned in to an
//! [`crate::ArticleStream`] covering the whole site.
//!
//! ```no_run
//! use futures::stream::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let site = extrablatt::Extrablatt::builder("https://some-news.com/")?.build().await?;
//!
//! let mut stream = site.into_stream();
//! while let Some(article) = stream.next().await {
//!     if let Ok(article) = article {
//!         println!("article '{:?}'", article.content.title)
//!     } else {
//!         println!("{:?}", article);
//!     }
//! }
//! #   Ok(())
//! # }
//! ```
//!
//! By default, extraction of valuable information is done using the
//! [`crate::DefaultExtractor`] which only uses the default implementation
//! of the [`crate::Extractor`] trait. To customize the content extraction,
//! the trait must be implemented for a custom extractor.
//!
//! ```no_run
//! use extrablatt::{Extractor, Language};
//! use extrablatt::select::{predicate::Attr, document::Document};
//! use std::borrow::Cow;
//!
//! pub struct MyExtractor {
//!     //...
//! }
//!
//! impl Extractor for MyExtractor {
//!
//!     fn text<'a>(&self, doc: &'a Document, lang: Language) -> Option<Cow<'a, str>> {
//!         // identify and extract the article's text from the `doc`,
//!         // e.g. by finding the very node that holds the text
//!         doc.find(Attr("id", "article")).next().map(|n| n.text().into())
//!     }
//! }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let extractor = MyExtractor{};
//! let article = extrablatt::Article::get_with_extractor(
//!     "http://example.com/interesting-article.html",
//!     &extractor,
//! )
//! .await
//! .unwrap();
//! #   Ok(())
//! # }
//! ```

/// Reexported to implement custom extractors.
pub use select;

pub use crate::article::{Article, PureArticle};
pub use crate::category::Category;
pub use crate::extrablatt::{ArticleStream, Config, Extrablatt, ExtrablattBuilder};
pub use crate::extract::{DefaultExtractor, Extractor};
pub use crate::language::Language;

pub mod article;
pub mod category;
pub mod clean;
pub mod date;
mod error;
pub mod extrablatt;
pub mod extract;
pub mod image;
pub mod language;
#[cfg(feature = "stopwords")]
mod stopwords;
pub mod text;
pub mod video;

pub mod nlp {

    pub(crate) static CATEGORY_STOPWORDS: [&str; 67] = [
        "about",
        "help",
        "privacy",
        "legal",
        "feedback",
        "sitemap",
        "sitemap.html",
        "profile",
        "account",
        "mobile",
        "facebook",
        "myspace",
        "twitter",
        "linkedin",
        "bebo",
        "friendster",
        "stumbleupon",
        "youtube",
        "vimeo",
        "store",
        "mail",
        "preferences",
        "maps",
        "password",
        "imgur",
        "flickr",
        "search",
        "subscription",
        "itunes",
        "siteindex",
        "events",
        "stop",
        "jobs",
        "careers",
        "newsletter",
        "subscribe",
        "academy",
        "shopping",
        "purchase",
        "site-map",
        "sitemap",
        "shop",
        "donate",
        "newsletter",
        "product",
        "advert",
        "info",
        "tickets",
        "coupons",
        "forum",
        "board",
        "archive",
        "browse",
        "howto",
        "how to",
        "faq",
        "terms",
        "charts",
        "services",
        "contact",
        "plus",
        "admin",
        "login",
        "signup",
        "register",
        "developer",
        "proxy",
    ];

    #[cfg(feature = "stopwords")]
    pub use crate::stopwords::*;
}
