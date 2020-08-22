use extrablatt::Article;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;
use web_sys::{Request, RequestInit, RequestMode, Response};

// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub async fn fetch_article(url: String) -> Result<JsValue, JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    console::log_1(&JsValue::from_str(&format!("requesting {}", url)));

    let request = Request::new_with_str_and_init(&url, &opts)?;

    request.headers().set("Accept", "*/*")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let txt = JsFuture::from(resp.text()?).await?;

    let doc = txt
        .as_string()
        .ok_or_else(|| JsValue::from_str("Received empty response"))?;

    let article = Article::new(&url, doc).map_err(|err| JsValue::from_str(&format!("{}", err)))?;

    JsValue::from_serde(&article.content).map_err(|err| JsValue::from_str(&format!("{}", err)))
}

#[wasm_bindgen]
pub async fn reqwest_article(url: String) -> Result<JsValue, JsValue> {
    let article = Article::content(&url)
        .await
        .map_err(|err| JsValue::from_str(&format!("{}", err)))?;
    JsValue::from_serde(&article).map_err(|err| JsValue::from_str(&format!("{}", err)))
}
