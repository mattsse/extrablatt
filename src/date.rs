use std::borrow::Cow;
use std::collections::HashMap;

use chrono::prelude::*;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;
use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Name, Predicate};

use lazy_static::lazy_static;

use crate::extract::NodeValueQuery;

lazy_static! {

    pub(crate) static ref RE_DATE_SEGMENTS_Y_M_D: Regex = Regex::new(r"(?mi)(19|20)\d\d[-\\/\.](0[1-9]|1[012]|([jfmasond]\w{2,7}))[-\\/\.](0[1-9]|[12][0-9]|3[01])").unwrap();

    pub(crate) static ref RE_DATE_SEGMENTS_M_D_Y: Regex = Regex::new(r"(?mi)(0[1-9]|1[012]|([jfmasond]\w{2,7}))[-\\/\.](0[1-9]|[12][0-9]|3[01])[-\\/\.](19|20)\d\d").unwrap();

    pub(crate) static ref RE_KEY_VALUE_PUBLISH_DATE: Regex = Regex::new(r#"(?mi)"\s*(([^"]|\w)*)?(date[-_\s]?(Published|created)|Pub(lish|lication)?[-_\s]?Date)\s*"\s*[:=]\s*"\s*(?P<date>[^"]*)\s*""#).unwrap();

    pub(crate) static ref RE_KEY_VALUE_MODIFIED_DATE: Regex = Regex::new(r#"(?mi)"\s*(([^"]|\w)*)?((date[\s_-]?modified|modified[\s_-]?date))\s*"\s*[:=]\s*"\s*(?P<date>[^"]*)\s*""#).unwrap();

    /// Common nodes that hold the article's modification date.
    pub(crate) static ref  MODIFIED_DATE_NODES: Vec<NodeValueQuery<'static>> = {
            let mut nodes = Vec::with_capacity(7);
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("property",  "article:modified"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("property",  "modified"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "ModificationDate"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "modification_date"),
             "content"));
             nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "lastmod"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("itemprop",  "dateModified"),
             "datetime"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "dateModified"),
             "content"));
             nodes
        };


    /// Common nodes that hold the article's publishing date.
    pub(crate) static ref  PUBLISH_DATE_NODES: Vec<NodeValueQuery<'static>> = {
            let mut nodes = Vec::with_capacity(12);
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("property",  "rnews:datePublished"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("property",  "article:published_time"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "OriginalPublicationDate"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("itemprop",  "datePublished"),
             "datetime"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("property",  "og:published_time"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "article_date_original"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "publication_date"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "sailthru.date"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "PublishDate"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "pubdate"),
             "content"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("pubdate",  "pubdate"),
             "datetime"));
            nodes.push(NodeValueQuery::new( Name("meta"), Attr("name",  "publish_date"),
             "content"));

            nodes.push(NodeValueQuery::new( Name("div"), Attr("id",  "taboola-feed-below-article"),
             "data-publishdate"));

             nodes
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
    /// Extract the date from the document using several options:
    ///
    /// 1. Look in the common `<meta>` nodes.
    /// 2. Regex the `<head>` node
    pub fn extract_from_doc(doc: &Document) -> Option<ArticleDate> {
        if let Some(published) =
            DateExtractor::extract_date(doc, &PUBLISH_DATE_NODES, &RE_KEY_VALUE_PUBLISH_DATE)
        {
            let last_updated = DateExtractor::extract_date(
                &doc,
                &MODIFIED_DATE_NODES,
                &RE_KEY_VALUE_MODIFIED_DATE,
            )
            .map(Update::DateTime);
            return Some(ArticleDate {
                published: Date::DateTime(published),
                last_updated,
            });
        }
        None
    }

    fn extract_date<'a>(
        doc: &Document,
        nodes: &[NodeValueQuery<'a>],
        regex: &Regex,
    ) -> Option<NaiveDateTime> {
        let mut date = {
            for node in nodes {
                if let Some(content) = doc
                    .find(node.name.and(node.attr))
                    .filter_map(|n| n.attr(node.content_name))
                    .next()
                {
                    if let Some(date) = DateExtractor::fuzzy_dtparse(content) {
                        return Some(date);
                    }
                }
            }
            None
        };

        if date.is_none() {
            // look for a "publicationDate":"2019..." in the doc str
            if let Some(head) = doc
                .find(Name("head"))
                .filter_map(|head| head.as_text())
                .next()
            {
                if let Some(capture) = regex.captures(head) {
                    date = capture
                        .name("date")
                        .and_then(|m| DateExtractor::fuzzy_dtparse(m.as_str()))
                }
            }
        }

        date
    }

    fn fuzzy_dtparse(s: &str) -> Option<NaiveDateTime> {
        let mut tzinfod = HashMap::new();
        tzinfod.insert("ET".to_string(), 14400);
        let parser = dtparse::Parser::default();
        parser
            .parse(
                s, None, None, true, /* turns on fuzzy mode */
                true, /* gives us the tokens that weren't recognized */
                None, false, &tzinfod,
            )
            .map(|(date, _, _)| date)
            .ok()
    }

    /// Extract the publishing timestamp from plain text using fuzzy searching
    /// with `dtparse`.
    pub fn extract_from_str(s: &str) -> Option<ArticleDate> {
        DateExtractor::fuzzy_dtparse(s).map(|published| ArticleDate {
            published: Date::DateTime(published),
            last_updated: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_modified() {
        let caps = RE_KEY_VALUE_MODIFIED_DATE
            .captures(r#""datemodified":"2019-12-05T15:34:34+0100""#)
            .unwrap();
        assert_eq!(
            caps.name("date").unwrap().as_str(),
            "2019-12-05T15:34:34+0100"
        )
    }

    #[test]
    fn publish_modified() {
        let caps = RE_KEY_VALUE_PUBLISH_DATE
            .captures(r#""datePublished":"2019-12-05T15:34:34+0100""#)
            .unwrap();
        assert_eq!(
            caps.name("date").unwrap().as_str(),
            "2019-12-05T15:34:34+0100"
        )
    }
}
