use regex::Regex;

use lazy_static::lazy_static;
use select::node::{Descendants, Node};
use select::predicate::{Name, Predicate};
use std::borrow::Cow;
use std::collections::HashSet;

lazy_static! {

    /// A [`Regex`] to determine whether a `Node`'s attribute should be ignored
    pub static ref RE_BAD_NODES_ATTR : Regex = Regex::new(r###"(?mi)^side$|combx|retweet|mediaarticlerelated|menucontainer|navbar|storytopbar-bucket|utility-bar|inline-share-tools|comment|PopularQuestions|contact|foot(er|note)?|cnn_strycaptiontxt|cnn_html_slideshow|cnn_strylftcntnt|links|meta$|shoutbox|sponsor|tags|socialnetworking|socialNetworking|cnnStryHghLght|cnn_stryspcvbx|^inset$|pagetools|post-attributes|welcome_form|contentTools2|the_answers|communitypromo|runaroundLeft|subscribe|vcard|articleheadings|date|^print$|popup|author-dropdown|tools|socialtools|byline|konafilter|breadcrumbs|^fn$|wp-caption-text|legende|ajoutVideo|timestamp|js_replies|[^-]facebook(-broadcasting)?|google|[^-]twitter|styln-briefing-block|read-more-link|js-body-read-more"###).unwrap();

}

pub const BAD_NODE_NAMES: &[&str; 5] = &["script", "style", "figcaption", "figure", "button"];

const ATTR_TO_CHECK: [&str; 3] = ["id", "class", "name"];

pub trait DocumentCleaner {
    /// Extract all textual content from the node, but ignore those nodes, that
    /// do not contain parts of the article.
    fn clean_node_text(&self, node: Node) -> String {
        fn recur_text<T: DocumentCleaner + ?Sized>(
            node: Node,
            txt: &mut String,
            cleaner: &T,
        ) -> bool {
            if cleaner.is_bad_node_name(node) {
                return false;
            }

            let mut txt_added = false;
            if cleaner.is_good_node(node) {
                let mut needs_ws = false;
                for child in node.children() {
                    if needs_ws {
                        txt.push(' ');
                        needs_ws = false;
                    }
                    if let Some(txt_fragment) = child.as_text().map(str::trim) {
                        if !txt_fragment.is_empty() {
                            txt.push_str(txt_fragment);
                            txt_added = true
                        }
                    } else if Name("a").matches(&child) {
                        // escape the content of a `<a>...</a>` tag that is embedded between
                        // text nodes
                        let mut a = String::new();
                        if recur_text(child, &mut a, cleaner) && txt_added {
                            txt.push(' ');
                            txt.push_str(&a);
                            needs_ws = true;
                        }
                    } else {
                        recur_text(child, txt, cleaner);
                    }
                }
                if (txt_added && is_para(node)) || needs_ws {
                    txt.push('\n');
                }
            }
            txt_added
        }

        let mut txt = String::new();
        recur_text(node, &mut txt, self);
        txt
    }

    /// Whether the node should be considered
    fn is_good_node(&self, node: Node) -> bool {
        !has_bad_attr(node)
    }

    /// Whether the node's should be ignored based on its name
    fn is_bad_node_name(&self, node: Node) -> bool {
        is_bad_node(node)
    }

    /// Create an iterator that yields every node that this cleaner considers
    /// good
    fn iter_clean_nodes<'a>(&'a self, node: Node<'a>) -> CleanNodeIter<'a, Self>
    where
        Self: Sized,
    {
        CleanNodeIter {
            cleaner: self,
            inner: node.descendants(),
        }
    }
}

/// An Iterator that only yields those nodes that the `cleaner` considers good.
pub struct CleanNodeIter<'a, T: DocumentCleaner> {
    cleaner: &'a T,
    inner: Descendants<'a>,
}

impl<'a, T: DocumentCleaner> Iterator for CleanNodeIter<'a, T> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.inner.next()?;
        if self.cleaner.is_bad_node_name(node) || !self.cleaner.is_good_node(node) {
            // skip every node under this bad node
            for ignore in node.descendants() {
                let next = self.inner.next()?;
                if ignore.index() != next.index() {
                    return Some(next);
                }
            }
        }
        Some(node)
    }
}

/// A standard implementation of a cleaner that only extracts good textual
/// content form the nodes descendants
pub struct DefaultDocumentCleaner;

impl DocumentCleaner for DefaultDocumentCleaner {}

/// A [`DocumentCleaner`] that uses names to detect tags to ignore by default
/// and a predicate to decide whether a tag should be ignore
#[derive(Debug)]
pub struct CommonCleaner<P>
where
    P: Fn(Node) -> bool,
{
    /// html tag names that are ignore by default
    pub bad_node_names: HashSet<Cow<'static, str>>,
    /// Predicate to decide whether a node can hold valid textual content
    pub is_good_node: P,
}

impl<P> CommonCleaner<P>
where
    P: Fn(Node) -> bool,
{
    /// Create a new Cleaner that ignores `BAD_TAG_NAMES` nodes by default and
    /// decides whether to extract text from a node based on the `is_good_node`
    /// predicate
    pub fn new(is_good_node: P) -> Self {
        Self::with_names(is_good_node, BAD_NODE_NAMES.iter().cloned())
    }

    /// Create a new Cleaner that ignores `bad_names` nodes by default and
    /// decides whether to extract text from a node based on the `is_good_node`
    /// predicate
    pub fn with_names<T, I>(is_good_node: P, bad_names: T) -> Self
    where
        T: IntoIterator<Item = I>,
        I: Into<Cow<'static, str>>,
    {
        Self {
            bad_node_names: bad_names.into_iter().map(I::into).collect(),
            is_good_node,
        }
    }
}

impl Default for CommonCleaner<for<'s> fn(Node<'s>) -> bool> {
    fn default() -> Self {
        Self::new(|n| !is_bad_node(n))
    }
}

impl<P> DocumentCleaner for CommonCleaner<P>
where
    P: Fn(Node) -> bool,
{
    fn is_good_node(&self, node: Node) -> bool {
        (self.is_good_node)(node)
    }

    /// Whether the node's should be ignored based on its name
    fn is_bad_node_name(&self, node: Node) -> bool {
        if let Some(n) = node.name().map(Cow::Borrowed) {
            self.bad_node_names.contains(&n)
        } else {
            true
        }
    }
}

pub fn is_bad_node(node: Node) -> bool {
    if let Some(n) = node.name() {
        BAD_NODE_NAMES.contains(&n)
    } else {
        false
    }
}

fn is_para(node: Node) -> bool {
    if let Some(n) = node.name() {
        let names = &[
            "blockquote",
            "dl",
            "div",
            "img",
            "ol",
            "p",
            "pre",
            "table",
            "ul",
        ];
        names.contains(&n)
    } else {
        false
    }
}

/// Ignore nodes that usually do not contain content for the article based on
/// attributes.
pub fn has_bad_attr(node: Node) -> bool {
    for attr in ATTR_TO_CHECK.iter() {
        if let Some(id) = node.attr(attr) {
            if RE_BAD_NODES_ATTR.is_match(id) {
                return true;
            }
        }
    }
    false
}
