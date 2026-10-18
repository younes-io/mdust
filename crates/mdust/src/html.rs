//! Hand-written matchers equivalent to markdown-it's HTML regexes.

use crate::utils::is_js_space;

/// Length of a JS `\s*` run starting at `i`.
fn skip_js_ws(s: &str, mut i: usize) -> usize {
    while let Some(c) = s[i..].chars().next() {
        if !is_js_space(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

fn attr_name_end(b: &[u8], i: usize) -> Option<usize> {
    if i >= b.len() || !(b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b':') {
        return None;
    }
    let mut j = i + 1;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b':' | b'.' | b'_' | b'-')) {
        j += 1;
    }
    Some(j)
}

fn attr_value_end(b: &[u8], i: usize) -> Option<usize> {
    let c = *b.get(i)?;
    if c == b'"' || c == b'\'' {
        let close = b[i + 1..].iter().position(|&x| x == c)?;
        return Some(i + 1 + close + 1);
    }
    let mut j = i;
    while j < b.len() && !matches!(b[j], b'"' | b'\'' | b'=' | b'<' | b'>' | b'`' | 0..=0x20) {
        j += 1;
    }
    if j == i {
        None
    } else {
        Some(j)
    }
}

fn tag_name_end(b: &[u8], i: usize) -> Option<usize> {
    if i >= b.len() || !b[i].is_ascii_alphabetic() {
        return None;
    }
    let mut j = i + 1;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-') {
        j += 1;
    }
    Some(j)
}

/// `open_tag` at `s[0]=='<'`. The regex backtracks, but each attribute alternative is
/// greedy and unambiguous enough that a single forward scan finds the same match.
fn open_tag(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = tag_name_end(b, 1)?;
    loop {
        let ws = skip_js_ws(s, i);
        if ws > i {
            if let Some(ne) = attr_name_end(b, ws) {
                let mut j = ne;
                let eq = skip_js_ws(s, j);
                if eq < b.len() && b[eq] == b'=' {
                    let v = skip_js_ws(s, eq + 1);
                    if let Some(ve) = attr_value_end(b, v) {
                        j = ve;
                    }
                }
                i = j;
                continue;
            }
        }
        i = ws;
        break;
    }
    if i < b.len() && b[i] == b'/' {
        i += 1;
    }
    if i < b.len() && b[i] == b'>' {
        Some(i + 1)
    } else {
        None
    }
}

fn close_tag(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 3 || b[1] != b'/' {
        return None;
    }
    let i = skip_js_ws(s, tag_name_end(b, 2)?);
    if i < b.len() && b[i] == b'>' {
        Some(i + 1)
    } else {
        None
    }
}

/// `HTML_OPEN_CLOSE_TAG_RE` anchored at the start of `s`.
pub fn open_close_tag(s: &str) -> Option<usize> {
    if !s.starts_with('<') {
        return None;
    }
    open_tag(s).or_else(|| close_tag(s))
}

/// Sorted so `block_name` can binary-search it.
pub const BLOCK_NAMES: &[&str] = &[
    "address", "article", "aside", "base", "basefont", "blockquote", "body", "caption", "center",
    "col", "colgroup", "dd", "details", "dialog", "dir", "div", "dl", "dt", "fieldset",
    "figcaption", "figure", "footer", "form", "frame", "frameset", "h1", "h2", "h3", "h4", "h5",
    "h6", "head", "header", "hr", "html", "iframe", "legend", "li", "link", "main", "menu",
    "menuitem", "nav", "noframes", "ol", "optgroup", "option", "p", "param", "search", "section",
    "summary", "table", "tbody", "td", "tfoot", "th", "thead", "title", "tr", "track", "ul",
];

const MAX_NAME_LEN: usize = 10;

/// The ASCII alphanumeric run at the start of `b`, lowercased into `buf`.
fn alnum_token(b: &[u8], buf: &mut [u8; MAX_NAME_LEN]) -> Option<usize> {
    let n = b.iter().take_while(|c| c.is_ascii_alphanumeric()).count();
    if n == 0 || n > MAX_NAME_LEN {
        return None;
    }
    for (d, s) in buf[..n].iter_mut().zip(&b[..n]) {
        *d = s.to_ascii_lowercase();
    }
    Some(n)
}

/// Byte length of the block-level tag name at the start of `b`, if `b` starts with one.
/// Names are alphanumeric and the regex lookahead admits no alphanumeric, so the whole
/// alphanumeric run must equal one name; alternation order never matters.
fn block_name(b: &[u8]) -> Option<usize> {
    let mut buf = [0u8; MAX_NAME_LEN];
    let n = alnum_token(b, &mut buf)?;
    let tok = core::str::from_utf8(&buf[..n]).ok()?;
    BLOCK_NAMES.binary_search(&tok).ok().map(|_| n)
}

fn raw_name(b: &[u8]) -> Option<usize> {
    let mut buf = [0u8; MAX_NAME_LEN];
    let n = alnum_token(b, &mut buf)?;
    match &buf[..n] {
        b"script" | b"pre" | b"style" | b"textarea" => Some(n),
        _ => None,
    }
}

/// `-->`, `?>`, `]]>`, and the raw-tag closers are too short for `str::contains`'s
/// searcher setup to pay off on one line.
fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    let (first, tail) = match needle.split_first() {
        Some(x) => x,
        None => return true,
    };
    let mut rest = hay;
    while let Some(i) = rest.iter().position(|&c| c == *first) {
        rest = &rest[i + 1..];
        if rest.starts_with(tail) {
            return true;
        }
    }
    false
}

fn contains_raw_closer(b: &[u8]) -> bool {
    let mut rest = b;
    while let Some(i) = rest.iter().position(|&c| c == b'<') {
        rest = &rest[i + 1..];
        if rest.first() == Some(&b'/') {
            if let Some(n) = raw_name(&rest[1..]) {
                if rest.get(1 + n) == Some(&b'>') {
                    return true;
                }
            }
        }
    }
    false
}

/// Which HTML_SEQUENCES entry (0..7) opens on `line`, if any.
pub fn block_sequence(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    if b.first() != Some(&b'<') {
        return None;
    }
    if let Some(n) = raw_name(&b[1..]) {
        match line[1 + n..].chars().next() {
            None => return Some(0),
            Some(c) if c == '>' || is_js_space(c) => return Some(0),
            _ => {}
        }
    }
    if b.starts_with(b"<!--") {
        return Some(1);
    }
    if b.starts_with(b"<?") {
        return Some(2);
    }
    if b.len() > 2 && b[1] == b'!' && b[2].is_ascii_alphabetic() {
        return Some(3);
    }
    if b.starts_with(b"<![CDATA[") {
        return Some(4);
    }
    let start = if b.get(1) == Some(&b'/') { 2 } else { 1 };
    if let Some(n) = block_name(&b[start..]) {
        let rest = &line[start + n..];
        let ok = match rest.chars().next() {
            None => true,
            Some('>') => true,
            Some('/') => rest[1..].starts_with('>'),
            Some(c) => is_js_space(c),
        };
        if ok {
            return Some(5);
        }
    }
    if let Some(e) = open_close_tag(line) {
        if line[e..].chars().all(is_js_space) {
            return Some(6);
        }
    }
    None
}

pub fn block_sequence_closes(seq: usize, line: &str) -> bool {
    let b = line.as_bytes();
    match seq {
        0 => contains_raw_closer(b),
        1 => contains_bytes(b, b"-->"),
        2 => contains_bytes(b, b"?>"),
        3 => b.contains(&b'>'),
        4 => contains_bytes(b, b"]]>"),
        _ => line.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_names_sorted() {
        assert!(BLOCK_NAMES.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn sequences() {
        assert_eq!(block_sequence("<DIV>"), Some(5));
        assert_eq!(block_sequence("<div/>"), Some(5));
        assert_eq!(block_sequence("</basefont"), Some(5));
        assert_eq!(block_sequence("<base>"), Some(5));
        assert_eq!(block_sequence("<divx>"), Some(6));
        assert_eq!(block_sequence("<div\u{a0}x"), Some(5));
        assert_eq!(block_sequence("<pre/>"), Some(6));
        assert_eq!(block_sequence("<Script x"), Some(0));
        assert_eq!(block_sequence("<scriptx"), None);
        assert_eq!(block_sequence("<!x"), Some(3));
        assert_eq!(block_sequence("<![CDATA["), Some(4));
        assert_eq!(block_sequence("<>"), None);
        assert!(block_sequence_closes(0, "x </SCRIPT> y"));
        assert!(!block_sequence_closes(0, "</script x>"));
        assert!(block_sequence_closes(1, "a --->"));
        assert!(!block_sequence_closes(1, "a -- >"));
        assert!(block_sequence_closes(4, "]]]>"));
    }
}
