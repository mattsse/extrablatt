use regex::Regex;

use lazy_static::lazy_static;
use select::node::Node;
use std::borrow::Cow;
use std::collections::HashSet;

lazy_static! {

    /// A [`Regex`] to determine whether a `Node`'s attribute should be ignored
    pub static ref RE_BAD_NODES_ATTR : Regex = Regex::new(r###"(?mi)^side$|combx|retweet|mediaarticlerelated|menucontainer|navbar|storytopbar-bucket|utility-bar|inline-share-tools|comment|PopularQuestions|contact|foot(er|note)?|cnn_strycaptiontxt|cnn_html_slideshow|cnn_strylftcntnt|links|meta$|shoutbox|sponsor|tags|socialnetworking|socialNetworking|cnnStryHghLght|cnn_stryspcvbx|^inset$|pagetools|post-attributes|welcome_form|contentTools2|the_answers|communitypromo|runaroundLeft|subscribe|vcard|articleheadings|date|^print$|popup|author-dropdown|tools|socialtools|byline|konafilter|breadcrumbs|^fn$|wp-caption-text|legende|ajoutVideo|timestamp|js_replies|[^-]facebook(-broadcasting)?|google|[^-]twitter"###).unwrap();

}

pub const BAD_TAG_NAMES: &[&'static str; 5] =
    &["script", "style", "figcaption", "figure", "button"];

const ATTR_TO_CHECK: [&str; 3] = ["id", "class", "name"];

pub trait DocumentCleaner {
    /// Extract all textual content from the node, but ignore those nodes, that
    /// do not contain parts of the article.
    fn clean_node_text(&self, node: &Node) -> String {
        fn recur_text<T: DocumentCleaner + ?Sized>(node: &Node, txt: &mut String) {
            if is_bad_node(node) {
                return;
            }
            if !has_bad_attr(node) {
                T::node_text(node, txt)
            }
            for child in node.children() {
                recur_text::<T>(&child, txt)
            }
        }

        let mut txt = String::new();
        recur_text::<Self>(node, &mut txt);
        txt
    }

    /// Extract the valuable textual content from the `node` and put it into
    /// `txt`
    fn node_text(node: &Node, txt: &mut String) {
        for txt_fragment in node
            .children()
            .filter_map(|n| n.as_text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            txt.push_str(txt_fragment);
            if is_para(node) {
                txt.push_str("\n");
            }
        }
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
    P: Fn(&Node) -> bool,
{
    /// html tag names that are ignore by default
    pub bad_node_names: HashSet<Cow<'static, str>>,
    /// Predicate to decide whether a node can hold valid textual content
    pub is_good_node: P,
}

impl<P> CommonCleaner<P>
where
    P: Fn(&Node) -> bool,
{
    /// Create a new Cleaner that ignores [`BAD_TAG_NAMES`] nodes by default and
    /// decides whether to extract text from a node based on the `is_good_node`
    /// predicate
    pub fn new(is_good_node: P) -> Self {
        Self::with_names(is_good_node, BAD_TAG_NAMES.iter().cloned())
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

    fn recur_text(&self, node: &Node, txt: &mut String) {
        if let Some(n) = node.name() {
            if self.bad_node_names.contains(&Cow::Borrowed(n)) {
                return;
            }
        }

        if (self.is_good_node)(node) {
            Self::node_text(node, txt)
        }

        for child in node.children() {
            self.recur_text(&child, txt)
        }
    }
}

impl Default for CommonCleaner<for<'r, 's> fn(&'r Node<'s>) -> bool> {
    fn default() -> Self {
        Self::new(|n| !is_bad_node(n))
    }
}

impl<P> DocumentCleaner for CommonCleaner<P>
where
    P: Fn(&Node) -> bool,
{
    fn clean_node_text(&self, node: &Node) -> String {
        let mut txt = String::new();
        self.recur_text(node, &mut txt);
        txt
    }
}

fn is_bad_node(node: &Node) -> bool {
    if let Some(n) = node.name() {
        BAD_TAG_NAMES.contains(&n)
    } else {
        false
    }
}

fn is_para(node: &Node) -> bool {
    if let Some(n) = node.name() {
        let names = &[
            "a",
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
fn has_bad_attr(node: &Node) -> bool {
    for attr in ATTR_TO_CHECK.iter() {
        if let Some(id) = node.attr(attr) {
            if RE_BAD_NODES_ATTR.is_match(id) {
                return true;
            }
        }
    }
    false
}
