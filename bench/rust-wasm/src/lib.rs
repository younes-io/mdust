use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

fn parser() -> &'static markdown_it::MarkdownIt {
    static P: OnceLock<markdown_it::MarkdownIt> = OnceLock::new();
    P.get_or_init(|| {
        let mut md = markdown_it::MarkdownIt::new();
        markdown_it::plugins::cmark::add(&mut md);
        markdown_it::plugins::html::add(&mut md);
        markdown_it::plugins::extra::strikethrough::add(&mut md);
        markdown_it::plugins::extra::tables::add(&mut md);
        md
    })
}

#[wasm_bindgen]
pub fn render(src: &str) -> String {
    parser().parse(src).render()
}

pub fn parse_only(src: &str) -> usize {
    let ast = parser().parse(src);
    let mut n = 0;
    ast.walk(|_, _| n += 1);
    n
}
