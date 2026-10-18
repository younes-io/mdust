mod block;
mod html;
mod inline;
mod links;
mod render;
mod tables;
mod url;
mod utils;

pub fn render(src: &str) -> String {
    let normalized;
    let src = if src.bytes().any(|c| c == b'\r' || c == 0) {
        normalized = src.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "\u{FFFD}");
        normalized.as_str()
    } else {
        src
    };
    let blocks = block::parse(src);
    let mut out = String::with_capacity(src.len() + src.len() / 4);
    render::render(&blocks, &mut out);
    out
}

#[doc(hidden)]
pub fn profile_blocks(src: &str) -> usize {
    block::parse(src).tokens.len()
}

#[doc(hidden)]
pub fn profile_inline_parse(src: &str) -> usize {
    let b = block::parse(src);
    let mut n = 0;
    for t in &b.tokens {
        if t.kind == block::Kind::Inline {
            n += inline::Inline::parse(b.content(t), &b.refs).tokens.len();
        }
    }
    n
}
