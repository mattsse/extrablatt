use regex::Regex;

use select::node::Node;

use lazy_static::lazy_static;

lazy_static! {

    pub static ref RE_BAD_NODES : Regex = Regex::new(r###"(?mi)^side$|combx|retweet|mediaarticlerelated|menucontainer|navbar|storytopbar-bucket|utility-bar|inline-share-tools|comment|PopularQuestions|contact|foot(er|note)?|cnn_strycaptiontxt|cnn_html_slideshow|cnn_strylftcntnt|links|meta$|shoutbox|sponsor|tags|socialnetworking|socialNetworking|cnnStryHghLght|cnn_stryspcvbx|^inset$|pagetools|post-attributes|welcome_form|contentTools2|the_answers|communitypromo|runaroundLeft|subscribe|vcard|articleheadings|date|^print$|popup|author-dropdown|tools|socialtools|byline|konafilter|breadcrumbs|^fn$|wp-caption-text|legende|ajoutVideo|timestamp|js_replies|[^-]facebook(-broadcasting)?|google|[^-]twitter"###).unwrap();

}

const ATTR_TO_CHECK: [&str; 3] = ["id", "class", "name"];

pub trait DocumentCleaner {
    /// Ignore nodes that usually do not contain content for the article.
    fn has_bad_attr(node: &Node) -> bool {
        for attr in ATTR_TO_CHECK.iter() {
            if let Some(id) = node.attr(attr) {
                if RE_BAD_NODES.is_match(id) {
                    return true;
                }
            }
        }
        false
    }

    /// Extract all textual content from the node, but ignore those nodes, that
    /// do not contain parts of the article.
    fn clean_node_text(node: &Node) -> String {
        fn recur_text<T: DocumentCleaner + ?Sized>(node: &Node, string: &mut String) {
            if !T::has_bad_attr(node) {
                if let Some(text) = node.as_text() {
                    string.push_str(text);
                }
            }
            for child in node.children() {
                recur_text::<T>(&child, string)
            }
        }

        let mut txt = String::new();
        recur_text::<Self>(node, &mut txt);
        txt
    }
}

/// A standard implementation of a cleaner that only extracts good textual
/// content form the nodes descendants
pub struct DefaultDocumentCleaner;

impl DocumentCleaner for DefaultDocumentCleaner {}
