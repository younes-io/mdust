use napi_derive::napi;

/// Renders CommonMark to HTML, byte-identical to markdown-it 15.0.3 with preset `default` and `{ html: true }`.
#[napi]
pub fn render(src: String) -> String {
    mdust::render(&src)
}
