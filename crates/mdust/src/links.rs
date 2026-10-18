//! Ports of markdown-it's `parseLinkDestination` and `parseLinkTitle`.
use crate::prelude::*;

use crate::utils::{unescape_all, unescape_all_into};

pub struct Dest<'a> {
    pub pos: usize,
    pub str: Cow<'a, str>,
}

pub fn parse_link_destination(s: &str, start: usize, max: usize) -> Option<Dest<'_>> {
    let b = s.as_bytes();
    let mut pos = start;
    if pos < b.len() && b[pos] == b'<' {
        pos += 1;
        while pos < max {
            match b[pos] {
                b'\n' | b'<' => return None,
                b'>' => return Some(Dest { pos: pos + 1, str: unescape_all(&s[start + 1..pos]) }),
                b'\\' if pos + 1 < max => pos += 2,
                _ => pos += 1,
            }
        }
        return None;
    }
    let mut level = 0;
    while pos < max {
        let c = b[pos];
        if c == b' ' || c < 0x20 || c == 0x7F {
            break;
        }
        if c == b'\\' && pos + 1 < max {
            if b[pos + 1] == b' ' {
                pos += 1;
                continue;
            }
            pos += 2;
            continue;
        }
        if c == b'(' {
            level += 1;
            if level > 32 {
                return None;
            }
        }
        if c == b')' {
            if level == 0 {
                break;
            }
            level -= 1;
        }
        pos += 1;
    }
    // `\` + multibyte char can step into the middle of a UTF-8 sequence
    while pos < b.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    if start == pos || level != 0 {
        return None;
    }
    Some(Dest { pos, str: unescape_all(&s[start..pos]) })
}

#[derive(Default)]
pub struct Title {
    pub ok: bool,
    pub can_continue: bool,
    pub pos: usize,
    pub str: String,
    pub marker: u8,
}

pub fn parse_link_title(s: &str, start: usize, max: usize, prev: Option<Title>) -> Title {
    let b = s.as_bytes();
    let mut pos = start;
    let mut start = start;
    let mut st = Title::default();
    if let Some(p) = prev {
        st.str = p.str;
        st.marker = p.marker;
    } else {
        if pos >= max {
            return st;
        }
        let mut marker = b[pos];
        if marker != b'"' && marker != b'\'' && marker != b'(' {
            return st;
        }
        start += 1;
        pos += 1;
        if marker == b'(' {
            marker = b')';
        }
        st.marker = marker;
    }
    while pos < max {
        let c = b[pos];
        if c == st.marker {
            st.pos = pos + 1;
            unescape_all_into(&mut st.str, &s[start..pos]);
            st.ok = true;
            return st;
        } else if c == b'(' && st.marker == b')' {
            return st;
        } else if c == b'\\' && pos + 1 < max {
            pos += 1;
        }
        pos += 1;
    }
    st.can_continue = true;
    let mut end = pos.min(b.len());
    while !s.is_char_boundary(end) {
        end += 1;
    }
    unescape_all_into(&mut st.str, &s[start..end]);
    st
}
