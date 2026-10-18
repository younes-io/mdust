#![no_std]
extern crate alloc;

mod prelude {
    pub use alloc::borrow::Cow;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

use prelude::*;

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
    let (src, blocks) = match block::parse(src) {
        Ok(blocks) => (src, blocks),
        Err(()) => {
            normalized = src.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "\u{FFFD}");
            let src = normalized.as_str();
            match block::parse(src) {
                Ok(blocks) => (src, blocks),
                Err(()) => unreachable!(),
            }
        }
    };
    let mut out = String::with_capacity(src.len() + src.len() / 4);
    render::render(&blocks, &mut out);
    out
}
