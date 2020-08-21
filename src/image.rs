use url::Url;

#[derive(Debug, Clone)]
pub struct Image {
    pub url: Url,
    pub caption: Option<String>,
}
