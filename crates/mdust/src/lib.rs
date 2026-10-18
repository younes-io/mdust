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

#[cfg(target_arch = "x86_64")]
mod x86;

/// Pins the x86 scanner level (1 SSE2, 2 SSSE3, 3 AVX2) so tests can cover
/// every path on one machine. No effect off x86_64.
#[doc(hidden)]
pub fn force_simd_level(level: u8) {
    #[cfg(target_arch = "x86_64")]
    x86::force(level);
    #[cfg(not(target_arch = "x86_64"))]
    let _ = level;
}

/// Current x86 scanner level (1 SSE2, 2 SSSE3, 3 AVX2), 0 off x86_64.
#[doc(hidden)]
pub fn simd_level() -> u8 {
    #[cfg(target_arch = "x86_64")]
    {
        x86::level()
    }
    #[cfg(not(target_arch = "x86_64"))]
    0
}
