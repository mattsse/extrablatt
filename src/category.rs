use crate::error::ExtrablattError;
use crate::language::Language;
use crate::{Article, ArticleStream, DefaultExtractor, Extractor};
use anyhow::Result;
use futures::Stream;
use std::borrow::Borrow;
use url::Url;

/// A category e.g. Politics or sports
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Category {
    /// The address for the category website.
    pub url: Url,
}

impl Category {
    pub fn new(url: Url) -> Self {
        Self { url }
    }

    /// Tries to identify the language by checking the url against known
    /// languages.
    ///
    /// # Example
    ///
    /// ```rust
    ///  use extrablatt::{Category, Language};
    ///  let category = Category::new("https://cnn.com/German/".parse().unwrap());
    ///  assert_eq!(category.language_hint(), Some(Language::German));
    /// ```
    pub fn language_hint(&self) -> Option<Language> {
        for lang in Language::known_languages() {
            let full_name = lang.full_name().to_lowercase();
            let id = lang.identifier();
            if let Some(domain) = &self.url.domain() {
                if domain.ends_with(&format!(".{}", id))
                    || domain.starts_with(&format!("{}.", id))
                    || domain.starts_with(&format!("{}.", full_name))
                {
                    return Some(lang.clone());
                }
            }
            if let Some(mut seg) = self.url.path_segments() {
                if seg.next().map(str::to_lowercase) == Some(full_name) {
                    return Some(lang.clone());
                }
            }
        }
        None
    }

    /// Fetch all article urls from the page this category's url points to and
    /// return a new stream of articles using the
    /// [`crate::DefaultExtractor`].
    pub async fn into_stream(
        self,
    ) -> Result<impl Stream<Item = std::result::Result<Article, ExtrablattError>>> {
        Ok(self.into_stream_with_extractor(DefaultExtractor).await?)
    }

    /// Fetch all article urls from the page this category's url points to and
    /// return a new stream of article using a designated
    /// [`crate::Extractor`].
    pub async fn into_stream_with_extractor<TExtractor: Extractor + Unpin>(
        self,
        extractor: TExtractor,
    ) -> Result<impl Stream<Item = std::result::Result<Article, ExtrablattError>>> {
        Ok(ArticleStream::new_with_extractor(self.url, extractor).await?)
    }
}

impl Borrow<str> for Category {
    fn borrow(&self) -> &str {
        self.url.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_lang_hint() {
        let category = Category::new(Url::parse("https://arabic.cnn.com/").unwrap());
        assert_eq!(category.language_hint(), Some(Language::Arabic));

        let category = Category::new(Url::parse("https://cnn.com/Arabic/").unwrap());
        assert_eq!(category.language_hint(), Some(Language::Arabic));

        let category = Category::new(Url::parse("https://cnn.com/Europe").unwrap());
        assert_eq!(category.language_hint(), None);
    }
}
