use std::collections::{HashMap, HashSet};

use std::ops::Deref;

use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Name, Predicate};

use crate::clean::{DefaultDocumentCleaner, DocumentCleaner};
use crate::video::VideoNode;
use crate::Language;
use url::Url;

/// Attribute key-value combinations to identify the root node for the textual
/// content of the article
pub const ARTICLE_BODY_ATTR: &[(&str, &str); 3] = &[
    ("itemprop", "articleBody"),
    ("data-testid", "article-body"),
    ("name", "articleBody"),
];

pub const PUNCTUATION: &str = r###",."'!?&-/:;()#$%*+<=>@[\]^_`{|}~"###;

pub trait TextContainer<'a> {
    fn first_children_text(&self) -> Option<&'a str>;
}

impl<'a> TextContainer<'a> for Node<'a> {
    fn first_children_text(&self) -> Option<&'a str> {
        self.children().find_map(|n| n.as_text())
    }
}

pub struct TextNodeFind<'a> {
    document: &'a Document,
    next: usize,
}

impl<'a> TextNodeFind<'a> {
    fn is_text_node(node: &Node<'a>) -> bool {
        Name("p").or(Name("pre").or(Name("td"))).matches(node)
    }

    fn is_bad(node: &Node<'a>) -> bool {
        Name("figure")
            .or(Name("media"))
            .or(Name("aside"))
            .matches(node)
    }

    fn new(document: &'a Document) -> Self {
        Self { document, next: 0 }
    }
}

impl<'a> Iterator for TextNodeFind<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Node<'a>> {
        while self.next < self.document.nodes.len() {
            let node = self.document.nth(self.next).unwrap();
            self.next += 1;
            if Self::is_bad(&node) {
                self.next += node.descendants().count();
            }
            if Self::is_text_node(&node) {
                return Some(node);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct ArticleTextNode<'a> {
    inner: Node<'a>,
}

impl<'a> ArticleTextNode<'a> {
    pub fn new(inner: Node<'a>) -> Self {
        Self { inner }
    }

    /// Extract the content from the node, but ignore those that not contain
    /// parts of the article
    pub fn clean_text(&self) -> String {
        DefaultDocumentCleaner.clean_node_text(self.inner)
    }

    /// Extract all of the images of the document.
    pub fn images(&self, base_url: Option<&Url>) -> Vec<Url> {
        let options = Url::options().base_url(base_url);
        self.inner
            .find(Name("img"))
            .filter_map(|n| n.attr("href").map(str::trim))
            .filter_map(|url| options.parse(url).ok())
            .collect()
    }

    /// Extract all the links within the node's descendants
    pub fn references(&self) -> Vec<Url> {
        let mut uniques = HashSet::new();
        self.find(Name("a"))
            .filter_map(|n| n.attr("href").map(str::trim))
            .filter(|href| uniques.insert(*href))
            .filter_map(|url| Url::parse(url).ok())
            .collect()
    }

    /// Extract all the nodes that hold video data
    pub fn videos(&self) -> Vec<VideoNode<'a>> {
        let mut videos: Vec<_> = self
            .inner
            .find(VideoNode::node_predicate())
            .map(VideoNode::new)
            .collect();

        videos.extend(
            self.inner
                .find(Name("embed"))
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

impl<'a> Deref for ArticleTextNode<'a> {
    type Target = Node<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ArticleTextNodeExtractor;

impl ArticleTextNodeExtractor {
    pub const MINIMUM_STOPWORD_COUNT: usize = 5;

    pub const MAX_STEPSAWAY_FROM_NODE: usize = 3;

    pub fn article_body_predicate() -> for<'r, 's> fn(&'r Node<'s>) -> bool {
        |node| {
            for (k, v) in ARTICLE_BODY_ATTR.iter().cloned() {
                if Attr(k, v).matches(node) {
                    return true;
                }
            }
            false
        }
    }

    pub fn calculate_best_node(doc: &Document, lang: Language) -> Option<ArticleTextNode> {
        let mut starting_boost = 1.0;

        let txt_nodes: Vec<_> = ArticleTextNodeExtractor::nodes_to_check(doc)
            .filter(|n| !ArticleTextNodeExtractor::is_high_link_density(n))
            .filter_map(|node| {
                if let Some(stats) = node
                    .first_children_text()
                    .and_then(|txt| lang.stopword_count(txt))
                {
                    if stats.stopword_count > 2 {
                        return Some((node, stats));
                    }
                }
                None
            })
            .collect();

        let mut nodes_scores = HashMap::with_capacity(txt_nodes.len());

        let nodes_number = txt_nodes.len();
        let negative_scoring = 0.0;
        let bottom_negativescore_nodes = nodes_number as f64 * 0.25;

        for (i, (node, stats)) in txt_nodes.iter().enumerate() {
            let mut boost_score = 0.0;

            if ArticleTextNodeExtractor::is_boostable(node, lang.clone()) {
                boost_score = (1.0 / starting_boost) * 50.0;
                starting_boost += 1.0;
            }

            if nodes_number > 15 {
                let score = (nodes_number - i) as f64;
                if score <= bottom_negativescore_nodes {
                    let booster = bottom_negativescore_nodes - score;
                    boost_score = booster.powf(2.0) * -1.0;

                    let negscore = boost_score.abs() + negative_scoring;
                    if negscore > 40.0 {
                        boost_score = 5.0;
                    }
                }
            }

            let upscore = stats.stopword_count + boost_score as usize;

            if let Some(parent) = node.parent() {
                let (score, cnt) = nodes_scores
                    .entry(parent.index())
                    .or_insert((0usize, 0usize));
                *score += upscore;
                *cnt += 1;

                // also update additional parent levels

                if let Some(parent_parent) = parent.parent() {
                    let (score, cnt) = nodes_scores
                        .entry(parent_parent.index())
                        .or_insert((0usize, 0usize));
                    *cnt += 1;
                    *score += upscore / 2;

                    if let Some(parent_2) = parent_parent.parent() {
                        let (score, cnt) = nodes_scores
                            .entry(parent_2.index())
                            .or_insert((0usize, 0usize));
                        *cnt += 1;
                        *score += upscore / 3;
                    }
                }
            }
        }

        let mut index = nodes_scores.keys().cloned().next();
        let mut top_score = 0;
        for (idx, (score, _)) in nodes_scores {
            if score > top_score {
                top_score = score;
                index = Some(idx);
            }
        }

        index.map(|i| ArticleTextNode::new(Node::new(doc, i).unwrap()))
    }

    /// Returns all nodes we want to search on like paragraphs and tables
    fn nodes_to_check(doc: &Document) -> impl Iterator<Item = Node> {
        TextNodeFind::new(doc)
    }

    /// A lot of times the first paragraph might be the caption under an image
    /// so we'll want to make sure if we're going to boost a parent node that it
    /// should be connected to other paragraphs, at least for the first n
    /// paragraphs so we'll want to make sure that the next sibling is a
    /// paragraph and has at least some substantial weight to it.
    fn is_boostable(node: &Node, lang: Language) -> bool {
        let mut steps_away = 0;
        while let Some(sibling) = node.prev().filter(|n| n.is(Name("p"))) {
            if steps_away >= ArticleTextNodeExtractor::MAX_STEPSAWAY_FROM_NODE {
                return false;
            }
            if let Some(stats) = sibling
                .first_children_text()
                .and_then(|txt| lang.stopword_count(txt))
            {
                if stats.stopword_count > ArticleTextNodeExtractor::MINIMUM_STOPWORD_COUNT {
                    return true;
                }
            }
            steps_away += 1;
        }
        false
    }

    /// Checks the density of links within a node, if there is a high link to
    /// text ratio, then the text is less likely to be relevant
    fn is_high_link_density(node: &Node) -> bool {
        let links = node.find(Name("a")).filter_map(|n| n.first_children_text());

        if let Some(words) = node.as_text().map(|s| s.split_whitespace()) {
            let words_number = words.count();
            if words_number == 0 {
                return true;
            }

            let (num_links, num_link_words) = links.fold((0usize, 0usize), |(links, sum), n| {
                (links + 1, sum + n.split_whitespace().count())
            });

            if num_links == 0 {
                return false;
            }

            let link_divisor = num_link_words as f64 / words_number as f64;
            let score = link_divisor * num_links as f64;

            score >= 1.0
        } else {
            links.count() > 0
        }
    }

    /// Returns an iterator over all words of the text.
    pub fn words(txt: &str) -> impl Iterator<Item = &str> {
        txt.split(|c: char| c.is_whitespace() || is_punctuation(c))
            .filter(|s| !s.is_empty())
    }
}

/// Whether the char is a punctuation.
pub fn is_punctuation(c: char) -> bool {
    PUNCTUATION.contains(c)
}

/// Statistic about words for a text.
#[derive(Debug, Clone)]
pub struct WordsStats {
    /// All the words.
    pub word_count: usize,
    /// All the stop words.
    pub stopword_count: usize,
}
