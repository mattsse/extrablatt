use futures::{pin_mut, stream::StreamExt};

use extrablatt::NewspaperBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let newspaper = NewspaperBuilder::new("https://cnn.com/")?.build().await?;

    let stream = newspaper.into_stream().await.take(3);
    pin_mut!(stream);
    while let Some(article) = stream.next().await {
        if let Ok(article) = article {
            println!("article '{:?}'", article.content.title)
        } else {
            println!("{:?}", article);
        }
    }

    Ok(())
}
