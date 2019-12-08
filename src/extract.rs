use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use log::{debug, info};
use regex::Regex;
use reqwest::Url;
use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Name, Predicate};
use url::Host;

use lazy_static::lazy_static;

use crate::article::{
    ArticleContent, ArticleUrl, ALLOWED_FILE_EXT, BAD_DOMAINS, BAD_SEGMENTS, GOOD_SEGMENTS,
};
use crate::date::{ArticleDate, DateExtractor, RE_DATE_SEGMENTS_M_D_Y, RE_DATE_SEGMENTS_Y_M_D};
use crate::error::ExtrablattError;
use crate::newspaper::Category;
use crate::stopwords::CATEGORY_STOPWORDS;
use crate::video::VideoNode;
use crate::Language;
use std::ops::Deref;

lazy_static! {
    /// Regex for cleaning author names.
    ///
    /// This strips leading "By " and also potential profile links.
    static ref RE_AUTHOR_NAME: Regex =
        Regex::new(r"(?mi)(By)?\s*((<|(&lt;))a([^>]*)(>|(&gt;)))?(?P<name>[a-z ,.'-]+)((<|(&lt;))\\/a(>|(&gt;)))?").unwrap();
}

pub(crate) struct NodeValueQuery<'a> {
    pub name: Name<&'a str>,
    /// The name of the attribute that holds the `value` of the `attribute` to
    /// check against.
    pub attr: Attr<&'a str, &'a str>,
    /// The name of the attribute that holds the requested value.
    pub content_name: &'a str,
}

impl<'a> NodeValueQuery<'a> {
    pub fn new(
        name: Name<&'a str>,
        attr: Attr<&'a str, &'a str>,
        content_name: &'a str,
    ) -> NodeValueQuery<'a> {
        NodeValueQuery {
            name,
            attr,
            content_name,
        }
    }
}

pub struct MetaNode<'a> {
    inner: Node<'a>,
}

impl<'a> MetaNode<'a> {
    pub fn attr<'b>(&'a self, attr: &'b str) -> Option<&'a str> {
        self.inner.attr(attr)
    }

    pub fn name_attr(&self) -> Option<&str> {
        self.attr("name")
    }

    pub fn property_attr(&self) -> Option<&str> {
        self.attr("property")
    }

    pub fn content_attr(&self) -> Option<&str> {
        self.attr("content")
    }

    pub fn value_attr(&self) -> Option<&str> {
        self.attr("value")
    }

    pub fn key(&self) -> Option<&str> {
        if let Some(prop) = self.property_attr() {
            Some(prop)
        } else {
            self.name_attr()
        }
    }

    pub fn value(&self) -> Option<&str> {
        if let Some(c) = self.content_attr() {
            Some(c)
        } else {
            self.value_attr()
        }
    }

    pub fn is_key_value(&self) -> bool {
        self.key().is_some() && self.value().is_some()
    }
}

impl<'a> Deref for MetaNode<'a> {
    type Target = Node<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub trait Extractor {
    /// Extract the article title.
    ///
    /// Assumptions:
    ///    - `og:title` usually contains the plain title, but shortened compared
    ///      to `<h1>`
    ///    - `<title>` tag is the most reliable, but often contains also the
    ///      newspaper name like: "Some title - The New York Times"
    ///    - `<h1>`, if properly detected, is the best since this is also
    ///      displayed to users)
    ///
    ///    Matching strategy:
    ///    1.  `<h1>` takes precedent over `og:title`
    ///    2. `og:title` takes precedent over `<title>`
    fn title<'a>(&self, doc: &'a Document) -> Option<Cow<'a, str>> {
        if let Some(title) = doc
            .find(Name("h1"))
            .filter_map(|node| node.as_text().map(str::trim))
            .next()
        {
            return Some(Cow::Borrowed(title));
        }

        if let Some(title) = self.meta_content(doc, Attr("property", "og:title")) {
            return Some(title);
        }

        if let Some(title) = self.meta_content(doc, Attr("name", "og:title")) {
            return Some(title);
        }

        if let Some(title) = doc.find(Name("title")).next() {
            return title.as_text().map(str::trim).map(Cow::Borrowed);
        }
        None
    }

    /// Extract all the listed authors for the article.
    fn authors<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        // look for author data in attributes
        let mut authors = HashSet::new();
        for &key in &["name", "rel", "itemprop", "class", "id"] {
            for &value in &["author", "byline", "dc.creator", "byl"] {
                authors.extend(
                    doc.find(Attr(key, value))
                        .filter_map(|n| n.as_text())
                        .filter_map(|name| RE_AUTHOR_NAME.captures(name))
                        .filter_map(|cap| cap.name("name"))
                        .map(|m| m.as_str().trim())
                        .flat_map(|name| name.split(" and ")),
                );
            }
        }
        authors.into_iter().map(Cow::Borrowed).collect()
    }

    /// When the article was published (and last updated).
    fn publishing_date(&self, doc: &Document, base_url: Option<&Url>) -> Option<ArticleDate> {
        if let Some(date) = DateExtractor::from_doc(doc) {
            return Some(date);
        }

        if let Some(url) = base_url {
            return DateExtractor::from_str(url.path());
        }

        None
    }

    /// Extract the favicon from a website.
    fn favicon(&self, doc: &Document, base_url: &Url) -> Option<Url> {
        let options = Url::options().base_url(Some(base_url));

        doc.find(Name("link").and(Attr("rel", "icon")))
            .filter_map(|node| node.attr("href"))
            .filter_map(|href| options.parse(href).ok())
            .next()
    }

    /// Extract content language from meta tag.
    fn meta_language(&self, doc: &Document) -> Option<Language> {
        let mut unknown_lang = None;

        if let Some(meta) = self.meta_content(doc, Attr("http-equiv", "Content-Language")) {
            match Language::from_str(&*meta) {
                Ok(lang) => return Some(lang),
                Err(lang) => {
                    unknown_lang = Some(lang);
                }
            }
        }

        if let Some(meta) = self.meta_content(doc, Attr("name", "lang")) {
            match Language::from_str(&*meta) {
                Ok(lang) => return Some(lang),
                Err(lang) => {
                    unknown_lang = Some(lang);
                }
            }
        }
        unknown_lang
    }

    /// Finds all `<meta>` nodes in the document.
    fn meta_data<'a>(&self, doc: &'a Document) -> Vec<MetaNode<'a>> {
        doc.find(Name("meta"))
            .map(|node| MetaNode { inner: node })
            .filter(MetaNode::is_key_value)
            .collect()
    }

    /// Extract a given meta content form document.
    fn meta_content<'a, 'b>(
        &self,
        doc: &'a Document,
        attr: Attr<&'b str, &'b str>,
    ) -> Option<Cow<'a, str>> {
        doc.find(Name("meta").and(attr))
            .filter_map(|node| node.attr("content").map(str::trim).map(Cow::Borrowed))
            .next()
    }

    /// Extract the thumbnail for the article.
    fn meta_thumbnail_url(&self, doc: &Document, base_url: &Url) -> Option<Url> {
        let options = Url::options().base_url(Some(base_url));
        if let Some(meta) = self.meta_content(doc, Attr("thumbnail", "thumbnailUrl")) {
            if let Ok(url) = options.parse(&*meta) {
                return Some(url);
            }
        }
        None
    }

    /// Extract the 'top img' as specified by the website.
    fn meta_img_url(&self, doc: &Document, base_url: &Url) -> Option<Url> {
        let options = Url::options().base_url(Some(base_url));

        if let Some(meta) = self.meta_content(doc, Attr("property", "og:image")) {
            if let Ok(url) = options.parse(&*meta) {
                return Some(url);
            }
        }

        doc.find(
            Name("link").and(
                Attr("rel", "img_src")
                    .or(Attr("rel", "image_src"))
                    .or(Attr("rel", "icon")),
            ),
        )
        .filter_map(|node| node.attr("href"))
        .filter_map(|href| options.parse(href).ok())
        .next()
    }

    /// Returns meta type of article, open graph protocol
    fn meta_type<'a>(&self, doc: &'a Document) -> Option<Cow<'a, str>> {
        self.meta_content(doc, Attr("property", "og:type"))
    }

    /// Returns site name of article, open graph protocol.
    fn meta_site_name<'a>(&self, doc: &'a Document) -> Option<Cow<'a, str>> {
        self.meta_content(doc, Attr("property", "og:site_name"))
    }

    /// If the article has meta description set in the source, use that
    fn meta_description<'a>(&self, doc: &'a Document) -> Option<Cow<'a, str>> {
        self.meta_content(doc, Attr("property", "description"))
    }

    /// If the article has meta keywords set in the source, use that.
    fn meta_keywords<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        self.meta_content(doc, Attr("property", "keywords"))
            .map(|s| match s {
                Cow::Owned(s) => s
                    .split(',')
                    .map(str::trim)
                    .map(ToString::to_string)
                    .map(Cow::Owned)
                    .collect(),
                Cow::Borrowed(s) => s.split(',').map(str::trim).map(Cow::Borrowed).collect(),
            })
            .unwrap_or_default()
    }

    /// Extract the `href` attribute for all `<a>` tags of the document.
    fn all_urls<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        doc.find(Name("a"))
            .filter_map(|n| n.attr("href").map(str::trim))
            .map(Cow::Borrowed)
            .collect()
    }

    fn article_urls(&self, doc: &Document, base_url: &Url) -> Vec<ArticleUrl> {
        let options = Url::options().base_url(Some(base_url));

        doc.find(Name("a"))
            .filter_map(|n| {
                if let Some(href) = n.attr("href").map(str::trim) {
                    Some((href, n.as_text().map(str::trim)))
                } else {
                    None
                }
            })
            .filter_map(|(link, title)| {
                options
                    .parse(link)
                    .map(|url| ArticleUrl::new(url, title))
                    .ok()
            })
            .filter(|article| Self::is_article(article, base_url))
            .collect()
    }

    /// Extract all of the images of the document.
    fn img_urls<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        doc.find(Name("img"))
            .filter_map(|n| n.attr("href").map(str::trim))
            .map(Cow::Borrowed)
            .collect()
    }

    /// First, perform basic format and domain checks like making sure the
    /// format of the url.
    ///
    ///
    /// We also filter out articles with a subdomain or first degree path on a
    /// registered bad keyword.
    fn is_article(article: &ArticleUrl, base_url: &Url) -> bool {
        if article.url.path().starts_with('#') {
            return false;
        }
        if !is_valid_domain(&article.url, base_url) {
            debug!("No valid article domain {:?}", article.url.domain());
            return false;
        }
        let mut path_segments = Vec::new();
        if let Some(segments) = article.url.path_segments() {
            for segment in segments {
                let segment = segment.to_lowercase().to_lowercase();
                if BAD_SEGMENTS.contains(&segment.as_str()) {
                    debug!("Bad segment in article path {:?}", article.url.domain());
                    return false;
                }
                if !segment.is_empty() {
                    path_segments.push(segment);
                }
            }

            if path_segments.is_empty() {
                return false;
            }

            let last_segment = path_segments.remove(path_segments.len() - 1);

            // check file extensions
            let mut iter = last_segment.rsplitn(2, '.');
            let after = iter.next();
            if let Some(segment) = iter.next() {
                let extension = after.unwrap().to_lowercase();
                if ALLOWED_FILE_EXT.contains(&extension.as_str()) {
                    // assumption that the article slug is also the filename like `some-title.html`
                    if segment.len() > 10 {
                        path_segments.push(segment.to_string());
                    }
                } else {
                    if extension.chars().all(char::is_numeric) {
                        // prevents false negatives for articles with a trailing id like `1.343234`
                        path_segments.push(last_segment);
                    } else {
                        debug!(
                            "Encountered blacklisted filetype {} on article {:?}",
                            last_segment, article.url
                        );
                        return false;
                    }
                }
            } else {
                path_segments.push(last_segment);
            }
        } else {
            debug!("Article url has no path segments: {:?}", article.url);
            return false;
        }

        // check if the url has a news slug title
        if let Some(last_segment) = path_segments.last() {
            let (dash_count, underscore_count) = count_dashes_and_underscores(last_segment);

            if dash_count > 4 || underscore_count > 4 {
                if let Some(Host::Domain(domain)) = article.url.host() {
                    if let Some(domain) = domain.split('.').rev().skip(1).next() {
                        let delim = if underscore_count > dash_count {
                            '_'
                        } else {
                            '-'
                        };
                        if last_segment.split(delim).all(|s| s != domain) {
                            return true;
                        }
                    } else {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }

        // check fo dates
        if RE_DATE_SEGMENTS_Y_M_D.is_match(article.url.path()) {
            return true;
        }
        if RE_DATE_SEGMENTS_M_D_Y.is_match(article.url.path()) {
            return true;
        }

        // allow good paths
        if path_segments.len() > 1 {
            for segment in path_segments.iter() {
                if GOOD_SEGMENTS.contains(&segment.as_str()) {
                    return true;
                }
            }
        }

        false
    }

    fn is_category(category: &Category, base_url: &Url) -> bool {
        if category.url.as_str().starts_with('#') {
            debug!(
                "Ignoring category url starting with '#': {:?}",
                category.url
            );
            return false;
        }

        if let Some(segments) = category.url.path_segments() {
            for (i, segment) in segments.enumerate() {
                if i > 0 {
                    return false;
                }
                if CATEGORY_STOPWORDS.contains(&segment) || segment == "index.html" {
                    return false;
                }
            }
        }

        if category.url.scheme() != base_url.scheme() {
            debug!(
                "Ignoring category url {:?} with unexpected scheme. Expected: {}, got: {} ",
                category.url,
                category.url.scheme(),
                base_url.scheme()
            );
            return false;
        }

        return is_valid_domain(&category.url, base_url);
    }

    /// Finds all of the top level urls, assuming that these are the category
    /// urls.
    fn categories(&self, doc: &Document, base_url: &Url) -> Vec<Category> {
        let options = Url::options().base_url(Some(base_url));
        let category_urls: HashSet<_> = self
            .all_urls(doc)
            .into_iter()
            .filter_map(|url| options.parse(&*url).ok())
            .map(|mut url| {
                url.set_query(None);
                url
            })
            .collect();

        category_urls
            .into_iter()
            .map(Category::new)
            .filter(|cat| Self::is_category(cat, base_url))
            .collect()
    }

    fn article_content<'a>(&self, doc: &'a Document) -> ArticleContent<'a> {
        unimplemented!()
    }

    ///  Return the article's canonical URL
    ///
    /// Gets the first available value of:
    ///   1. The rel=canonical tag
    ///   2. The og:url tag
    fn canonical_link(&self, doc: &Document) -> Option<Url> {
        if let Some(link) = doc
            .find(Name("link").and(Attr("rel", "canonical")))
            .filter_map(|node| node.attr("href"))
            .next()
        {
            return Url::parse(link).ok();
        }

        if let Some(meta) = self.meta_content(doc, Attr("property", "og:url")) {
            return Url::parse(&*meta).ok();
        }

        None
    }

    fn videos<'a>(&self, doc: &'a Document) -> Vec<VideoNode<'a>> {
        let mut videos: Vec<_> = doc
            .find(Name("iframe").or(Name("object").or(Name("video"))))
            .map(VideoNode::new)
            .collect();

        videos.extend(
            doc.find(Name("embed"))
                .filter(|n| {
                    if let Some(parent) = n.parent() {
                        parent.name() != Some("object")
                    } else {
                        false
                    }
                })
                .map(VideoNode::new),
        );

        videos
    }
}

pub fn count_dashes_and_underscores<T: AsRef<str>>(s: T) -> (usize, usize) {
    let s = s.as_ref();
    let dash_count = s.chars().fold(0, |mut dashes, c| {
        if c == '-' {
            dashes += 1;
        }
        dashes
    });

    let underscore_count = s.chars().fold(0, |mut dashes, c| {
        if c == '_' {
            dashes += 1;
        }
        dashes
    });

    (dash_count, underscore_count)
}

/// Checks whether the `url`'s domain contains at least the `base_url` domain as
/// a subdomain.
pub fn is_valid_domain(url: &Url, base_url: &Url) -> bool {
    // check for subdomains
    if let Some(Host::Domain(domain)) = url.host() {
        let base_subdomains = base_url.domain().map(|x| x.split('.').collect::<Vec<_>>());

        if let Some(parent_domains) = &base_subdomains {
            let candidate_domains: Vec<_> = domain.split('.').collect();

            if parent_domains.iter().all(|d| candidate_domains.contains(d)) {
                // check for mobile
                if candidate_domains.iter().any(|d| *d == "m" || *d == "i") {
                    return false;
                }
                if candidate_domains
                    .iter()
                    .all(|d| !CATEGORY_STOPWORDS.contains(d) && !BAD_DOMAINS.contains(d))
                {
                    return true;
                }
            }
        }
    } else {
        // check host equality
        return base_url.host() == url.host();
    }
    false
}

#[derive(Debug, Default)]
pub struct DefaultExtractor;

impl Extractor for DefaultExtractor {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_regex() {
        let m = RE_AUTHOR_NAME
            .captures("By &lt;a href=&quot;/profiles/meg-wagner&quot;&gt;Joseph Kelley&lt;/a&gt;")
            .unwrap()
            .name("name")
            .unwrap();
        assert_eq!(m.as_str(), "Joseph Kelley");

        let m = RE_AUTHOR_NAME
            .captures("By <a href=&quot;/profiles/meg-wagner&quot;>Joseph Kelley</a>")
            .unwrap()
            .name("name")
            .unwrap();
        assert_eq!(m.as_str(), "Joseph Kelley");

        let m = RE_AUTHOR_NAME
            .captures("Joseph Kelley")
            .unwrap()
            .name("name")
            .unwrap();
        assert_eq!(m.as_str(), "Joseph Kelley");

        let m = RE_AUTHOR_NAME
            .captures("By Joseph Kelley")
            .unwrap()
            .name("name")
            .unwrap();
        assert_eq!(m.as_str(), "Joseph Kelley");

        let m = RE_AUTHOR_NAME
            .captures("J\'oseph-Kelley")
            .unwrap()
            .name("name")
            .unwrap();
        assert_eq!(m.as_str(), "J\'oseph-Kelley");
    }

    #[test]
    fn detect_articles() {
        macro_rules! assert_articles {
            ($base:expr => $($url:expr,)*) => {
                let base_url = Url::parse($base).unwrap();
                $(
                    let article = ArticleUrl::new(Url::parse($url).unwrap(), None);
                    assert!(DefaultExtractor::is_article(&article, &base_url));
                )*
            };
        }

        assert_articles!(
               "https://extrablatt.com" =>
               "https://extrablatt.com/politics/live-news/some-title-12-05-2019/index.html",
               "https://extrablatt.com/2019/12/04/us/politics/some-title.html",
               "https://extrablatt.com/2019/12/04/us/politics/some-title.html",
               "https://www.extrablatt.com/graphics/2019/investigations/some-title/",
               "https://extrablatt.com/2019/12/06/uk/some-longer-title-with-dashes/index.html",
               "https://www.extrablatt.com/politik/some-longer-title-with-dashes-interview-1.347823?reduced=true",
               "https://www.extrablatt.com/auto/some_longer_title_with_underscores_1300105.html",
        );
    }
}
