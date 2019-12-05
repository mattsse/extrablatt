use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use log::{debug, info};
use reqwest::Url;
use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Name, Predicate};
use url::Host;

use crate::article::ArticleUrl;
use crate::date::{ArticleDate, DateExtractor};
use crate::newspaper::Category;
use crate::stopwords::CATEGORY_STOPWORDS;
use crate::Language;
use lazy_static::lazy_static;
use regex::Regex;
use std::str::FromStr;

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

pub trait Extractor {
    // TODO this should contain a type Document, the implement the defaultextractor
    // with type select::Document

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

    fn article_urls<'a>(&self, doc: &'a Document, base_url: &Url) -> Vec<ArticleUrl<'a>> {
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
                // TODO concat for article links
                Some(link)
            });

        unimplemented!()
    }

    /// Extract all of the images of the document.
    fn img_urls<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        doc.find(Name("img"))
            .filter_map(|n| n.attr("href").map(str::trim))
            .map(Cow::Borrowed)
            .collect()
    }

    fn create_article_url<T: AsRef<str>>(base_url: &Url, path: T) -> anyhow::Result<Url> {
        let options = Url::options().base_url(Some(base_url));

        let url = path.as_ref();
        if url.starts_with('#') {
            debug!("Ignoring url starting with '#': {:?}", url);
        }

        unimplemented!()
    }

    /// Finds all of the top level urls, assuming that these are the category
    /// urls.
    fn categories(&self, doc: &Document, base_url: &Url) -> Vec<Category> {
        let options = Url::options().base_url(Some(base_url));
        let base_subdomains = base_url.domain().map(|x| x.split('.').collect::<Vec<_>>());
        let candidates = self.all_urls(doc);

        let mut category_urls = Vec::new();

        for url in candidates {
            if url.starts_with('#') {
                debug!("Ignoring category url starting with '#': {:?}", url);
                continue;
            }

            match options.parse(&*url) {
                Ok(url) => {
                    if url.scheme() != base_url.scheme() {
                        debug!(
                            "Ignoring category url {:?} with unexpected scheme. Expected: {}, got: {} ",
                            url,
                            url.scheme(),
                            base_url.scheme()
                        );
                        continue;
                    }

                    // check for subdomains
                    if let Some(Host::Domain(domain)) = url.host() {
                        if let Some(parent_domains) = &base_subdomains {
                            let candidate_domains: Vec<_> = domain.split('.').collect();

                            if parent_domains.iter().all(|d| candidate_domains.contains(d)) {
                                // check for mobile
                                if candidate_domains.iter().any(|d| *d == "m" || *d == "i") {
                                    debug!("Ignoring category url for mobile subdomain: {:?}", url);
                                } else {
                                    if candidate_domains
                                        .iter()
                                        .any(|d| CATEGORY_STOPWORDS.contains(d))
                                    {
                                        debug!("Ignoring category url {:?} for containing a blacklisted subdomain.", url);
                                    } else {
                                        category_urls.push(url);
                                    }
                                }
                            } else {
                                debug!("Ignoring category url due to base url domain mismatch. Expected subdomain for {:?}, got: {:?}", base_url.domain(), url.domain());
                            }
                        } else {
                            debug!("Ignoring category url due to base url domain mismatch. Expected Host::Domain, got: {:?}",url.host());
                        }
                    } else {
                        // check host equality
                        if base_url.host() == url.host() {
                            category_urls.push(url);
                        }
                    }
                }
                Err(e) => {
                    debug!("Ignoring category {:?}: {:?}", url, e);
                }
            }
        }

        let category_urls: HashSet<_> = category_urls
            .into_iter()
            .filter(|candidate| {
                if let Some(segments) = candidate.path_segments() {
                    segments
                        .filter(|s| !CATEGORY_STOPWORDS.contains(s) && *s != "index.html")
                        .count()
                        == 1
                } else {
                    false
                }
            })
            .map(|mut url| {
                url.set_query(None);

                url
            })
            .collect();

        category_urls.into_iter().map(Category::new).collect()
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
}
