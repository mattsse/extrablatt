use extrablatt::nlp::add_two;
use extrablatt::Article;



#[tokio::test]
 async fn arti() -> Result<(), Box<dyn std::error::Error>> {
       let content = Article::builder("https://www.znbc.co.zm/news/laz-suspends-mwanawasa/")?.get().await?.content;
     println!(" This is the Content {:?}",content.text);
    Ok(())
}
