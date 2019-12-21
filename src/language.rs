use crate::stopwords::*;
use crate::text::{TextExtractor, WordsStats};
#[cfg(feature = "serde0")]
use serde::{Deserialize, Serialize};
use std::slice::Iter;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde0", derive(Serialize, Deserialize))]
pub enum Language {
    Arabic,
    Russian,
    Dutch,
    German,
    English,
    Spanish,
    French,
    Hebrew,
    Italian,
    Korean,
    Norwegian,
    Persian,
    Polish,
    Portuguese,
    Swedish,
    Hungarian,
    Finnish,
    Danish,
    Chinese,
    Indonesian,
    Vietnamese,
    Swahili,
    Turkish,
    Greek,
    Ukrainian,
    Other(String),
}

impl Language {
    /// All known languages.
    pub fn known_languages() -> Iter<'static, Language> {
        static LANGUAGES: [Language; 25] = [
            Language::Arabic,
            Language::Russian,
            Language::Dutch,
            Language::German,
            Language::English,
            Language::Spanish,
            Language::French,
            Language::Hebrew,
            Language::Italian,
            Language::Korean,
            Language::Norwegian,
            Language::Persian,
            Language::Polish,
            Language::Portuguese,
            Language::Swedish,
            Language::Hungarian,
            Language::Finnish,
            Language::Danish,
            Language::Chinese,
            Language::Indonesian,
            Language::Vietnamese,
            Language::Swahili,
            Language::Turkish,
            Language::Greek,
            Language::Ukrainian,
        ];
        LANGUAGES.iter()
    }

    pub fn identifier(&self) -> &str {
        match self {
            Language::Arabic => "ar",
            Language::Russian => "ru",
            Language::Dutch => "nl",
            Language::German => "de",
            Language::English => "en",
            Language::Spanish => "es",
            Language::French => "fr",
            Language::Hebrew => "he",
            Language::Italian => "it",
            Language::Korean => "ko",
            Language::Norwegian => "no",
            Language::Persian => "fa",
            Language::Polish => "pl",
            Language::Portuguese => "pt",
            Language::Swedish => "sv",
            Language::Hungarian => "hu",
            Language::Finnish => "fi",
            Language::Danish => "da",
            Language::Chinese => "zh",
            Language::Indonesian => "id",
            Language::Vietnamese => "vi",
            Language::Swahili => "sw",
            Language::Turkish => "tr",
            Language::Greek => "el",
            Language::Ukrainian => "uk",
            Language::Other(s) => s.as_str(),
        }
    }

    pub fn full_name(&self) -> &str {
        match self {
            Language::Arabic => "Arabic",
            Language::Russian => "Russian",
            Language::Dutch => "Dutch",
            Language::German => "German",
            Language::English => "English",
            Language::Spanish => "Spanish",
            Language::French => "French",
            Language::Hebrew => "Hebrew",
            Language::Italian => "Italian",
            Language::Korean => "Korean",
            Language::Norwegian => "Norwegian",
            Language::Persian => "Persian",
            Language::Polish => "Polish",
            Language::Portuguese => "Portuguese",
            Language::Swedish => "Swedish",
            Language::Hungarian => "Hungarian",
            Language::Finnish => "Finnish",
            Language::Danish => "Danish",
            Language::Chinese => "Chinese",
            Language::Indonesian => "Indonesian",
            Language::Vietnamese => "Vietnamese",
            Language::Swahili => "Swahili",
            Language::Turkish => "Turkish",
            Language::Greek => "Greek",
            Language::Ukrainian => "Ukrainian",
            Language::Other(s) => s.as_str(),
        }
    }

    /// Counts the number of stopwords in the text, if stopwords for that
    /// language are available.
    pub fn stopword_count(&self, txt: &str) -> Option<WordsStats> {
        if let Some(stopwords) = self.stopwords() {
            let (word_count, stopword_count) = TextExtractor::words(txt).fold(
                (0usize, 0usize),
                |(word_count, mut stopword_count), word| {
                    if stopwords.contains(&word) {
                        stopword_count += 1;
                    }

                    (word_count + 1, stopword_count)
                },
            );
            Some(WordsStats {
                word_count,
                stopword_count,
            })
        } else {
            None
        }
    }

    /// Get the stopwords for a language.
    pub fn stopwords(&self) -> Option<&[&str]> {
        match self {
            Language::Arabic => Some(&ARABIC_STOPWORDS),
            Language::Russian => Some(&RUSSIAN_STOPWORDS),
            Language::Dutch => Some(&DUTCH_STOPWORDS),
            Language::German => Some(&GERMAN_STOPWORDS),
            Language::English => Some(&ENGLISH_STOPWORDS),
            Language::Spanish => Some(&SPANISH_STOPWORDS),
            Language::French => Some(&FRENCH_STOPWORDS),
            Language::Hebrew => Some(&HEBREW_STOPWORDS),
            Language::Italian => Some(&ITALIAN_STOPWORDS),
            Language::Korean => Some(&KOREAN_STOPWORDS),
            Language::Norwegian => Some(&NORWEGIAN_STOPWORDS),
            Language::Persian => Some(&PERSIAN_STOPWORDS),
            Language::Polish => Some(&POLISH_STOPWORDS),
            Language::Portuguese => Some(&PORTUGUESE_STOPWORDS),
            Language::Swedish => Some(&SWEDISH_STOPWORDS),
            Language::Hungarian => Some(&HUNGARIAN_STOPWORDS),
            Language::Finnish => Some(&FINNISH_STOPWORDS),
            Language::Danish => Some(&DANISH_STOPWORDS),
            Language::Chinese => Some(&CHINESE_STOPWORDS),
            Language::Indonesian => Some(&INDONESIAN_STOPWORDS),
            Language::Vietnamese => Some(&VIETNAMESE_STOPWORDS),
            Language::Swahili => Some(&SWAHILI_STOPWORDS),
            Language::Turkish => Some(&TURKISH_STOPWORDS),
            Language::Greek => Some(&GREEK_STOPWORDS),
            Language::Ukrainian => Some(&UKRAINIAN_STOPWORDS),
            Language::Other(_) => None,
        }
    }
}

impl FromStr for Language {
    type Err = Language;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ar" | "arabic" => Ok(Language::Arabic),
            "ru" | "russian" => Ok(Language::Russian),
            "nl" | "dutch" => Ok(Language::Dutch),
            "de" | "german" => Ok(Language::German),
            "en" | "english" => Ok(Language::English),
            "es" | "spanish" => Ok(Language::Spanish),
            "fr" | "french" => Ok(Language::French),
            "he" | "hebrew" => Ok(Language::Hebrew),
            "it" | "italian" => Ok(Language::Italian),
            "ko" | "korean" => Ok(Language::Korean),
            "no" | "norwegian" => Ok(Language::Norwegian),
            "fa" | "persian" => Ok(Language::Persian),
            "pl" | "polish" => Ok(Language::Polish),
            "pt" | "portuguese" => Ok(Language::Portuguese),
            "sv" | "swedish" => Ok(Language::Swedish),
            "hu" | "hungarian" => Ok(Language::Hungarian),
            "fi" | "finnish" => Ok(Language::Finnish),
            "da" | "danish" => Ok(Language::Danish),
            "zh" | "chinese" => Ok(Language::Chinese),
            "id" | "indonesian" => Ok(Language::Indonesian),
            "vi" | "vietnamese" => Ok(Language::Vietnamese),
            "sw" | "swahili" => Ok(Language::Swahili),
            "tr" | "turkish" => Ok(Language::Turkish),
            "el" | "greek" => Ok(Language::Greek),
            "uk" | "ukrainian" => Ok(Language::Ukrainian),
            s => Err(Language::Other(s.to_string())),
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}
