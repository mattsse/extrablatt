use crate::Language;
use select::document::Document;
use select::node::Node;
use select::predicate::{Name, Predicate};
use std::collections::HashMap;

pub const PUNCTUATION: &'static str = r###",."'!?&-/:;()#$%*+<=>@[\]^_`{|}~"###;

/// Statistic about words for a text.
#[derive(Debug, Clone)]
pub struct WordsStats {
    /// All the words.
    pub word_count: usize,
    /// All the stop words.
    pub stopword_count: usize,
}

pub struct TextExtractor;

impl TextExtractor {
    pub const MINIMUM_STOPWORD_COUNT: usize = 5;

    pub const MAX_STEPSAWAY_FROM_NODE: usize = 3;

    pub fn calculate_best_node(doc: &Document, lang: Language) -> Option<Node> {
        let mut starting_boost = 1.0;
        let cnt = 0usize;

        let txt_nodes: Vec<_> = TextExtractor::nodes_to_check(doc)
            .filter(TextExtractor::is_high_link_density)
            .filter_map(|node| {
                if let Some(stats) = node.as_text().and_then(|txt| lang.stopword_count(txt)) {
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

            if TextExtractor::is_boostable(node, lang.clone()) {
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

                if let Some(parent_parent) = parent.parent() {
                    let (score, cnt) = nodes_scores
                        .entry(parent_parent.index())
                        .or_insert((0usize, 0usize));
                    *cnt += 1;
                    *score += upscore / 2;
                }
            }
        }

        let mut index = nodes_scores.keys().map(|i| *i).next();
        let mut top_score = 0;
        for (idx, (score, _)) in nodes_scores {
            if score > top_score {
                top_score = score;
                index = Some(idx);
            }
        }

        index.map(|i| Node::new(doc, i).unwrap())
    }

    /// Returns all nodes we want to search on like paragraphs and tables
    fn nodes_to_check(doc: &Document) -> impl Iterator<Item = Node> {
        doc.find(Name("p").or(Name("pre").or(Name("td"))))
    }

    /// A lot of times the first paragraph might be the caption under an image
    /// so we'll want to make sure if we're going to boost a parent node that it
    /// should be connected to other paragraphs, at least for the first n
    /// paragraphs so we'll want to make sure that the next sibling is a
    /// paragraph and has at least some substantial weight to it.
    fn is_boostable(node: &Node, lang: Language) -> bool {
        let mut steps_away = 0;
        while let Some(sibling) = node.prev().filter(|n| n.is(Name("p"))) {
            if steps_away >= TextExtractor::MAX_STEPSAWAY_FROM_NODE {
                return false;
            }
            if let Some(stats) = sibling.as_text().and_then(|txt| lang.stopword_count(txt)) {
                if stats.stopword_count > TextExtractor::MINIMUM_STOPWORD_COUNT {
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
        let links = node.find(Name("a")).filter_map(|n| n.as_text());

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
            return links.count() != 0;
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
