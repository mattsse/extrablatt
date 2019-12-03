use chrono::prelude::*;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use lazy_static::lazy_static;
use select::node::Node;
use std::borrow::Cow;
use std::collections::HashMap;

pub struct MetaTag<'a> {
    /// The name of the attribute that holds the `value`
    attribute: Cow<'a, str>,
    /// Value of the `attribute` to check against.
    value: Cow<'a, str>,
    /// The name of the attribute that holds the requested value.
    content: Cow<'a, str>,
}
impl<'a> MetaTag<'a> {
    pub fn from_str(attribute: &'a str, value: &'a str, content: &'a str) -> MetaTag<'a> {
        MetaTag {
            attribute: Cow::Borrowed(attribute),
            value: Cow::Borrowed(value),
            content: Cow::Borrowed(content),
        }
    }
}

lazy_static! {
    /// Common `<meta>` tags that hold the article's publishing date.
    ///
    /// For a [`MetaTag::from_str("property", "pubdate", "content")`] tag, the str value of the
    /// publishing date would be expected to be located in the `content` attribute of the following
    /// meta tag `<meta property="pubdate", content="2019-12-03T15:19:31.282Z">`
    ///
    static ref  PUBLISH_DATE_META_TAGS: Vec<MetaTag<'static>> = {
            let mut tags = Vec::with_capacity(11);
            tags.push(MetaTag::from_str( "property",  "rnews:datePublished",
             "content"));
            tags.push(MetaTag::from_str( "property",  "article:published_time",
             "content"));
            tags.push(MetaTag::from_str( "name",  "OriginalPublicationDate",
             "content"));
            tags.push(MetaTag::from_str( "itemprop",  "datePublished",
             "datetime"));
            tags.push(MetaTag::from_str( "property",  "og:published_time",
             "content"));
            tags.push(MetaTag::from_str( "name",  "article_date_original",
             "content"));
            tags.push(MetaTag::from_str( "name",  "publication_date",
             "content"));
            tags.push(MetaTag::from_str( "name",  "sailthru.date",
             "content"));
            tags.push(MetaTag::from_str( "name",  "PublishDate",
             "content"));
            tags.push(MetaTag::from_str( "pubdate",  "pubdate",
             "datetime"));
            tags.push(MetaTag::from_str( "name",  "publish_date",
             "content"));
             tags
        };
}

#[derive(Debug, Clone)]
pub enum Date {
    /// The ISO 8601 date, a pair of year, month and day of the year.
    Date(NaiveDate),
    /// ISO 8601 combined date and time without timezone
    DateTime(NaiveDateTime),
}

#[derive(Debug, Clone)]
pub enum Update {
    /// The ISO 8601 date, a pair of year, month and day of the year.
    Date(NaiveDate),
    /// ISO 8601 combined date and time without timezone
    DateTime(NaiveDateTime),
    /// Time of day.
    Time(NaiveTime),
}

#[derive(Debug, Clone)]
pub struct ArticleDate {
    /// When the article was first published.
    pub published: Date,
    /// Last time the article was updated.
    pub last_updated: Option<Update>,
}

pub struct DateExtractor;

impl DateExtractor {
    /// Extract the dates using the `<meta>` tags of the head node.
    fn extract_from_head(head: Node) -> Option<ArticleDate> {
        unimplemented!()
    }

    /// Extract the publishing timestamp from plain text using fuzzy option of
    /// `dateparse` for now.
    // TODO needs propper parsing impl that handles also updates
    fn extract_from_text(s: &str) -> Option<ArticleDate> {
        let mut tzinfod = HashMap::new();
        tzinfod.insert("ET".to_string(), 14400);
        let parser = dtparse::Parser::default();
        parser
            .parse(
                s, None, None, true, /* turns on fuzzy mode */
                true, /* gives us the tokens that weren't recognized */
                None, false, &tzinfod,
            )
            .ok()
            .map(|(published, _, _)| ArticleDate {
                published: Date::DateTime(published),
                last_updated: None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;
    use dtparse::Parser;
    use std::collections::HashMap;

    #[test]
    fn parse_dates() {}
}
