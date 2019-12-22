use std::path::PathBuf;

use structopt::StructOpt;

use extrablatt::Newspaper;

#[allow(missing_docs)]
#[derive(Debug, StructOpt)]
#[structopt(name = "extrablatt", about = "News article scraping and curation.")]
#[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
enum App {
    #[structopt(name = "paper", about = "Extract all articles from a news source.")]
    Paper(Paper),
    #[structopt(name = "article", about = "Extract a single article")]
    Article,
}

#[derive(Debug, StructOpt)]
struct Paper {
    #[structopt(
        long = "name",
        short = "n",
        help = "The newspaper to extract papers from."
    )]
    name: String,
    #[structopt(
        long = "output",
        short = "o",
        help = "Output directory where to store downloaded articles.",
        parse(from_os_str)
    )]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, StructOpt)]
pub struct Config {
    /// Number of word tokens in the text.
    min_word_count: Option<usize>,
    /// Number of sentence tokens.
    min_sentence_count: Option<usize>,
    /// Number of chars for the text's title.
    max_title_len: Option<usize>,
    /// Number of chars for the text.
    max_text_len: Option<usize>,
    /// Number of keywords for the text.
    max_keywords: Option<usize>,
    /// Number of Authors.
    max_authors: Option<usize>,
    /// Number of chars of the summary.
    max_summary_len: Option<usize>,
    /// Number of sentences.
    max_summary_sentences: Option<usize>,
    /// Whether to extract images from the site.
    fetch_images: Option<bool>,
    /// Whether to kee the html of the article.
    keep_article_html: Option<bool>,
    /// The user-agent used for requests.
    browser_user_agent: Option<String>,
    /// Timeout for requests.
    request_timeout: Option<usize>,
    /// Whether to capture only 2XX responses or failures as well.
    http_success_only: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::try_init()?;
    let app = App::from_args();

    dbg!(app);
    Ok(())
}
