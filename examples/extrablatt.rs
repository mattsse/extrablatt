use futures::stream::StreamExt;

use extrablatt::Newspaper;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let newspaper = Newspaper::builder("https://cnn.com/")?.build().await?;

    let mut stream = newspaper.into_stream().skip(5).take(2);
    while let Some(article) = stream.next().await {
        if let Ok(article) = article {
            dbg!((article.url, article.content));
        } else {
            println!("{:?}", article);
        }
    }

    Ok(())
}
