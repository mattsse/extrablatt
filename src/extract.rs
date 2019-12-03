use crate::date::ArticleDate;
use reqwest::Url;
use select::document::Document;
use uri::Uri;

pub trait Extractor {
    /// Extract the article title and analyze it.
    fn title(&self, doc: &Document) -> Option<String>;

    /// Extract all the listed authors for the article.
    fn authors(&self, doc: &Document) -> Option<Vec<String>>;

    /// When the article was published (and last updated).
    fn publishing_date(&self, url: &Url, doc: &Document) -> Option<ArticleDate>;

    /// Extract the favicon from a website.
    fn favicon(&self, doc: &Document) -> Option<Uri>;

    /// Extract content language from meta tag.
    fn meta_lang(&self, doc: &Document) -> Option<String> {
        unimplemented!()
    }

    fn meta_content(&self, doc: &Document, metaname: &str) -> Option<String> {
        unimplemented!()
    }

    /// Extract the 'top img' as specified by the website.
    fn meta_img_url(&self, doc: &Document) -> Option<Url>;

    /// Returns meta type of article, open graph protocol
    fn meta_type(&self, doc: &Document) -> Option<String> {
        self.meta_content(doc, r###"meta[property="og:type"]"###)
    }

    /// Returns site name of article, open graph protocol.
    fn meta_site_name(&self, doc: &Document) -> Option<String> {
        self.meta_content(doc, r###"meta[property="og:site_name"]"###)
    }

    /// If the article has meta description set in the source, use that
    fn meta_description(&self, doc: &Document) -> Option<String> {
        self.meta_content(doc, "meta[name=description]")
    }

    /// If the article has meta keywords set in the source, use that.
    fn meta_keywords(&self, doc: &Document) -> Option<String> {
        self.meta_content(doc, "meta[name=keywords]")
    }

    /// Extract all of urls of the document.
    fn urls(&self, doc: &Document) -> Option<Vec<Url>> {
        unimplemented!()
    }

    /// Extract all of the images of the document.
    fn img_urls(&self, doc: &Document) -> Option<Vec<Url>> {
        unimplemented!()
    }

    fn category_urls(&self, url: &Url, doc: &Document) -> Option<Vec<String>>;

    ///  Return the article's canonical URL
    ///
    /// Gets the first available value of:
    ///   1. The rel=canonical tag
    ///   2. The og:url tag
    fn canonical_link(&self, url: &Url, doc: &Document) -> Option<String> {
        unimplemented!()
    }
}

pub struct DefaultExtractor {}

impl Extractor for DefaultExtractor {
    fn title(&self, doc: &Document) -> Option<String> {
        unimplemented!()
    }

    fn authors(&self, doc: &Document) -> Option<Vec<String>> {
        unimplemented!()
    }

    fn publishing_date(&self, url: &Url, doc: &Document) -> Option<ArticleDate> {
        unimplemented!()
    }

    fn favicon(&self, doc: &Document) -> Option<Uri> {
        unimplemented!()
    }

    fn meta_img_url(&self, doc: &Document) -> Option<Url> {
        unimplemented!()
    }

    fn category_urls(&self, url: &Url, doc: &Document) -> Option<Vec<String>> {
        unimplemented!()
    }
}
