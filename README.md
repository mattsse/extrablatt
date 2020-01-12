extrablatt
=====================
[![Crates.io](https://img.shields.io/crates/v/extrablatt.svg)](https://crates.io/crates/extrablatt)
[![Documentation](https://docs.rs/extrablatt/badge.svg)](https://docs.rs/extrablatt)

Still WIP.

Article scraping & curation.
Inspired by [newspaper](https://github.com/codelucas/newspaper).

Scraping is done using [select.rs]("https://github.com/utkarshkukreti/select.rs").

## Features

* News url identification
* Text extraction
* Top image extraction
* All image extraction
* Keyword extraction
* Author extraction

Adaptable for specific Newspapers via the `Extractor` trait.


## Example

Extract all Articles from a site.

````rust
use extrablatt::{Language, Newspaper};
use futures::{StreamExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let newspaper = Newspaper::builder("https://cnn.com/")?.build().await?;

    let mut stream = newspaper.into_stream().await;
    
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

## Documentation

[https://docs.rs/extrablatt](https://docs.rs/extrablatt)

## License

Licensed under either of these:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)
   
