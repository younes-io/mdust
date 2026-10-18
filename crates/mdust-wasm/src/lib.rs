use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render(src: &str) -> String {
    mdust::render(src)
}
