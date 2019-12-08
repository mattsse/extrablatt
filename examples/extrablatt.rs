use extrablatt::newspaper::NewspaperBuilder;
use extrablatt::{Extractor, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let mut newspaper = NewspaperBuilder::new("https://cnn.it/")?
        .language(Language::Italian)
        .build()
        .await?;

    let stream = newspaper.as_stream();

    //    let categories = newspaper.extract_articles().await;
    //
    //    for (cat, doc) in categories {}

    //    dbg!(categories);

    Ok(())
}
