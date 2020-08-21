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
