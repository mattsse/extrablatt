use std::borrow::Cow;
use std::collections::HashSet;

use log::{debug, info};
use reqwest::Url;
use select::document::Document;
use select::predicate::Name;
use url::Host;

use crate::date::ArticleDate;
use crate::newspaper::Category;
use crate::stopwords::CATEGORY_STOPWORDS;
use crate::Language;

pub trait Extractor: Sized {
    /// Extract the article title and analyze it.
    fn title(&self, doc: &Document) -> Option<String> {
        unimplemented!()
    }

    /// Extract all the listed authors for the article.
    fn authors(&self, doc: &Document) -> Option<Vec<String>> {
        unimplemented!()
    }

    /// When the article was published (and last updated).
    fn publishing_date(&self, url: &Url, doc: &Document) -> Option<ArticleDate>;

    /// Extract the favicon from a website.
    fn favicon(&self, doc: &Document) -> Option<Url>;

    /// Extract content language from meta tag.
    fn meta_lang(&self, doc: &Document) -> Option<Language> {
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
    fn urls<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        doc.find(Name("a"))
            .filter_map(|n| n.attr("href"))
            .map(Cow::Borrowed)
            .collect()
    }

    /// Extract all of the images of the document.
    fn img_urls<'a>(&self, doc: &'a Document) -> Vec<Cow<'a, str>> {
        doc.find(Name("img"))
            .filter_map(|n| n.attr("href"))
            .map(Cow::Borrowed)
            .collect()
    }

    /// Finds all of the top level urls, assuming that these are the category
    /// urls.
    // TODO change api to supply base url?
    fn category_urls(&self, doc: &Document) -> Vec<Category>;

    ///  Return the article's canonical URL
    ///
    /// Gets the first available value of:
    ///   1. The rel=canonical tag
    ///   2. The og:url tag
    fn canonical_link(&self, url: &Url, doc: &Document) -> Option<String> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct DefaultExtractor {
    /// Base url of the newspaper.
    base_url: Url,
}

impl DefaultExtractor {
    pub fn new(base_url: Url) -> Self {
        Self { base_url }
    }
}

impl Extractor for DefaultExtractor {
    fn publishing_date(&self, url: &Url, doc: &Document) -> Option<ArticleDate> {
        unimplemented!()
    }

    fn favicon(&self, doc: &Document) -> Option<Url> {
        unimplemented!()
    }

    fn meta_img_url(&self, doc: &Document) -> Option<Url> {
        unimplemented!()
    }

    fn category_urls(&self, doc: &Document) -> Vec<Category> {
        let options = Url::options();
        let base_url = options.base_url(Some(&self.base_url));
        let base_subdomains = self
            .base_url
            .domain()
            .map(|x| x.split('.').collect::<Vec<_>>());
        let candidates = self.urls(doc);

        let mut category_urls = Vec::new();

        for url in candidates {
            if url.starts_with('#') {
                debug!("Ignoring category url starting with '#': {:?}", url);
                continue;
            }

            match base_url.parse(&*url) {
                Ok(url) => {
                    if url.scheme() != self.base_url.scheme() {
                        debug!(
                            "Ignoring category url {:?} with unexpected scheme. Expected: {}, got: {} ",
                            url,
                            url.scheme(),
                            self.base_url.scheme()
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
                                debug!("Ignoring category url due to base url domain mismatch. Expected subdomain for {:?}, got: {:?}", self.base_url.domain(), url.domain());
                            }
                        } else {
                            debug!("Ignoring category url due to base url domain mismatch. Expected Host::Domain, got: {:?}",url.host());
                        }
                    } else {
                        // check host equality
                        if self.base_url.host() == url.host() {
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
}
