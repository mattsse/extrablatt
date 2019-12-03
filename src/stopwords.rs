use crate::language::Language;
use anyhow::Result;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::path::Path;

macro_rules! stop_words {
    ($($name:ident $lang:tt,)*) => {
        lazy_static! {
            $(
                static ref $name: StopWords = StopWords::read_language_stopwords(Language::$lang).unwrap();
            )*
        }
    };
}

stop_words!(
    ARABIC_STOPWORDS Arabic,
    RUSSIAN_STOPWORDS Russian,
    DUTCH_STOPWORDS Dutch,
    GERMAN_STOPWORDS German,
    ENGLISH_STOPWORDS English,
    SPANISH_STOPWORDS Spanish,
    FRENCH_STOPWORDS French,
    HEBREW_STOPWORDS Hebrew,
    ITALIAN_STOPWORDS Italian,
    KOREAN_STOPWORDS Korean,
    NORWEGIAN_STOPWORDS Norwegian,
    PERSIAN_STOPWORDS Persian,
    POLISH_STOPWORDS Polish,
    PORTUGUESE_STOPWORDS Portuguese,
    SWEDISH_STOPWORDS Swedish,
    HUNGARIAN_STOPWORDS Hungarian,
    FINNISH_STOPWORDS Finnish,
    DANISH_STOPWORDS Danish,
    CHINESE_STOPWORDS Chinese,
    INDONESIAN_STOPWORDS Indonesian,
    VIETNAMESE_STOPWORDS Vietnamese,
    SWAHILI_STOPWORDS Swahili,
    TURKISH_STOPWORDS Turkish,
    GREEK_STOPWORDS Greek,
    UKRAINIAN_STOPWORDS Ukrainian,
);

#[derive(Debug, Clone)]
pub struct StopWords {
    pub language: Language,
    words: HashSet<String>,
}

impl Deref for StopWords {
    type Target = HashSet<String>;

    fn deref(&self) -> &Self::Target {
        &self.words
    }
}

impl StopWords {
    /// Read the `Stopwords` for the `language` from the corresponding file.
    pub fn read_language_stopwords(language: Language) -> Result<Self> {
        let file_name = format!("stopwords-{}.txt", language.identifier());

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources/stopwords")
            .join(file_name);

        let mut f = fs::File::open(path)?;
        let file = BufReader::new(&f);
        let words: Result<HashSet<_>, _> = file.lines().collect();

        Ok(Self {
            language,
            words: words?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_stopwords() {
        let stopwords = StopWords::read_language_stopwords(Language::English).unwrap();
        assert!(!stopwords.is_empty());
    }

    #[test]
    fn lazy_stopwords() {
        let words = &ARABIC_STOPWORDS;
        let words = &RUSSIAN_STOPWORDS;
        let words = &DUTCH_STOPWORDS;
        let words = &GERMAN_STOPWORDS;
        let words = &ENGLISH_STOPWORDS;
        let words = &SPANISH_STOPWORDS;
        let words = &FRENCH_STOPWORDS;
        let words = &HEBREW_STOPWORDS;
        let words = &ITALIAN_STOPWORDS;
        let words = &KOREAN_STOPWORDS;
        let words = &NORWEGIAN_STOPWORDS;
        let words = &PERSIAN_STOPWORDS;
        let words = &POLISH_STOPWORDS;
        let words = &PORTUGUESE_STOPWORDS;
        let words = &SWEDISH_STOPWORDS;
        let words = &HUNGARIAN_STOPWORDS;
        let words = &FINNISH_STOPWORDS;
        let words = &DANISH_STOPWORDS;
        let words = &CHINESE_STOPWORDS;
        let words = &INDONESIAN_STOPWORDS;
        let words = &VIETNAMESE_STOPWORDS;
        let words = &SWAHILI_STOPWORDS;
        let words = &TURKISH_STOPWORDS;
        let words = &GREEK_STOPWORDS;
        let words = &UKRAINIAN_STOPWORDS;
    }
}
