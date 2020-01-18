use std::path::PathBuf;

use futures::{stream, StreamExt};
use structopt::StructOpt;
use url::Url;

use extrablatt::article::PureArticle;
use extrablatt::{Article, Category, Config, Newspaper};

#[allow(missing_docs)]
#[derive(Debug, StructOpt)]
#[structopt(name = "extrablatt", about = "News article scraping and curation.")]
#[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
enum App {
    #[structopt(name = "paper", about = "Extract all articles from a news source.")]
    Paper {
        #[structopt(name = "url", help = "The main url of the news outlet.")]
        url: Url,
        #[structopt(flatten)]
        opts: Opts,
    },
    #[structopt(name = "article", about = "Extract single articles")]
    Article {
        #[structopt(name = "urls", help = "The urls of the articles to download.")]
        urls: Vec<Url>,
        #[structopt(
            long = "output",
            short = "o",
            help = "The file to store downloaded articles as json.",
            parse(from_os_str)
        )]
        output: Option<PathBuf>,
    },
    #[structopt(name = "category", about = "Extract all article found on the page")]
    Category {
        #[structopt(
            name = "url",
            help = "The url of the category to extract and download articles."
        )]
        url: Url,
        #[structopt(
            long = "output",
            short = "o",
            help = "The file to store downloaded articles as json.",
            parse(from_os_str)
        )]
        output: Option<PathBuf>,
    },
}

impl App {
    async fn run(self) -> anyhow::Result<()> {
        let (out, articles) = match self {
            App::Paper { url, opts } => {
                let config = opts.as_config();
                (
                    opts.output,
                    Newspaper::builder(url)
                        .unwrap()
                        .config(config)
                        .build()
                        .await?
                        .into_stream()
                        .collect::<Vec<_>>()
                        .await
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?,
                )
            }
            App::Article { urls, output } => (
                output,
                stream::iter(
                    urls.into_iter()
                        .map(|url| Article::builder(url).unwrap().get()),
                )
                .buffer_unordered(10)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?,
            ),
            App::Category { url, output } => (
                output,
                Category::new(url)
                    .into_stream()
                    .await?
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Self::write(
            out,
            articles.into_iter().map(Article::drop_document).collect(),
        )
        .await
    }

    /// Writes the articles as json.
    ///
    /// If a output file is configured, then the articles will be stored there,
    /// otherwise to std::out.
    async fn write(out: Option<PathBuf>, articles: Vec<PureArticle>) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&articles)?;
        if let Some(out) = out {
            tokio::fs::write(out, json).await?;
        } else {
            println!("{}", json);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, StructOpt)]
pub struct Opts {
    #[structopt(name = "max-title", help = "Number of word tokens in the text.")]
    min_word_count: Option<usize>,
    #[structopt(name = "max-title", help = "Max number of chars for the text's title.")]
    max_title_len: Option<usize>,
    #[structopt(name = "max-text", help = "Number of chars for the text.")]
    max_text_len: Option<usize>,
    #[structopt(name = "max-keywords", help = "Maximum number of Keywords.")]
    max_keywords: Option<usize>,
    #[structopt(name = "max-authors", help = "Maximum number of Authors.")]
    max_authors: Option<usize>,
    #[structopt(name = "user-agent", help = "The user-agent used for requests.")]
    user_agent: Option<String>,
    #[structopt(
        name = "success-only",
        help = "Whether to capture only 2XX responses or failures as well."
    )]
    http_success_only: Option<bool>,
    #[structopt(
        long = "output",
        short = "o",
        help = "The file to store downloaded articles as json.",
        parse(from_os_str)
    )]
    output: Option<PathBuf>,
}

impl Opts {
    fn as_config(&self) -> Config {
        let mut config = Config::builder();
        if let Some(min_word_count) = self.min_word_count {
            config = config.min_word_count(min_word_count);
        }
        if let Some(max_title_len) = self.max_title_len {
            config = config.max_title_len(max_title_len);
        }
        if let Some(max_text_len) = self.max_text_len {
            config = config.max_text_len(max_text_len);
        }
        if let Some(max_keywords) = self.max_keywords {
            config = config.max_keywords(max_keywords);
        }
        if let Some(max_authors) = self.max_authors {
            config = config.max_authors(max_authors);
        }
        if let Some(user_agent) = self.user_agent.clone() {
            config = config.user_agent(user_agent);
        }
        if let Some(http_success_only) = self.http_success_only {
            config = config.http_success_only(http_success_only);
        }

        config.build()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(App::from_args().run().await?)
}
