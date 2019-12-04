use std::slice::Iter;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn languages() -> Iter<'static, Language> {
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
        LANGUAGES.into_iter()
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
}

impl<T: AsRef<str>> From<T> for Language {
    fn from(s: T) -> Self {
        match s.as_ref() {
            "ar" | "arabic" => Language::Arabic,
            "ru" | "russian" => Language::Russian,
            "nl" | "dutch" => Language::Dutch,
            "de" | "german" => Language::German,
            "en" | "english" => Language::English,
            "es" | "spanish" => Language::Spanish,
            "fr" | "french" => Language::French,
            "he" | "hebrew" => Language::Hebrew,
            "it" | "italian" => Language::Italian,
            "ko" | "korean" => Language::Korean,
            "no" | "norwegian" => Language::Norwegian,
            "fa" | "persian" => Language::Persian,
            "pl" | "polish" => Language::Polish,
            "pt" | "portuguese" => Language::Portuguese,
            "sv" | "swedish" => Language::Swedish,
            "hu" | "hungarian" => Language::Hungarian,
            "fi" | "finnish" => Language::Finnish,
            "da" | "danish" => Language::Danish,
            "zh" | "chinese" => Language::Chinese,
            "id" | "indonesian" => Language::Indonesian,
            "vi" | "vietnamese" => Language::Vietnamese,
            "sw" | "swahili" => Language::Swahili,
            "tr" | "turkish" => Language::Turkish,
            "el" | "greek" => Language::Greek,
            "uk" | "ukrainian" => Language::Ukrainian,
            s => Language::Other(s.to_string()),
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}
