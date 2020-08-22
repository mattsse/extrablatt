extrablatt
=====================
[![Crates.io](https://img.shields.io/crates/v/extrablatt.svg)](https://crates.io/crates/extrablatt)
[![Documentation](https://docs.rs/extrablatt/badge.svg)](https://docs.rs/extrablatt)

Customizable article scraping & curation library and CLI.
Also runs in Wasm. Basic Wasm example with some CORS limitations [https://mattsse.github.io/extrablatt/](https://mattsse.github.io/extrablatt/).


Inspired by [newspaper](https://github.com/codelucas/newspaper).
Html Scraping is done via [select.rs]("https://github.com/utkarshkukreti/select.rs").

## Features

* News url identification
* Text extraction
* Top image extraction
* All image extraction
* Keyword extraction
* Author extraction
* Publishing date
* References

Customizable for specific news sites/layouts via the `Extractor` trait.

## Documentation

Full Documentation [https://docs.rs/extrablatt](https://docs.rs/extrablatt)

## Example

Extract all Articles from news outlets.

````rust
use extrablatt::Extrablatt;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let site = Extrablatt::builder("https://some-news.com/")?.build().await?;

    let mut stream = site.into_stream();
    
    while let Some(article) = stream.next().await {
        if let Ok(article) = article {
            println!("article '{:?}'", article.content.title)
        } else {
            println!("{:?}", article);
        }
    }

    Ok(())
}
````

## Command Line

### Install

```bash
cargo install extrablatt --features="cli"
```

### Usage 

```text
USAGE:
    extrablatt <SUBCOMMAND>

SUBCOMMANDS:
    article     Extract a set of articles
    category    Extract all articles found on the page
    help        Prints this message or the help of the given subcommand(s)
    site        Extract all articles from a news source.

```

### Extract a set of specific articles and store the result as json

````bash
extrablatt article "https://www.example.com/article1.html", "https://www.example.com/article2.html" -o "articles.json"
````

## License

Licensed under either of these:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)
   
