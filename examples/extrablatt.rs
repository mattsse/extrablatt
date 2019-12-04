use extrablatt::newspaper::NewspaperBuilder;
use extrablatt::{Extractor, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let newspaper = NewspaperBuilder::new("https://cnn.it/")?
        .language(Language::Italian)
        .build()
        .await?;

    let categories = newspaper
        .extractor
        .category_urls(&newspaper.main_page, &newspaper.base_url);

    dbg!(categories);

    Ok(())
}
