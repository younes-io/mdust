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

pub const BLOCK_NAMES: &[&str] = &[
    "address", "article", "aside", "base", "basefont", "blockquote", "body", "caption", "center",
    "col", "colgroup", "dd", "details", "dialog", "dir", "div", "dl", "dt", "fieldset",
    "figcaption", "figure", "footer", "form", "frame", "frameset", "h1", "h2", "h3", "h4", "h5",
    "h6", "head", "header", "hr", "html", "iframe", "legend", "li", "link", "main", "menu",
    "menuitem", "nav", "noframes", "ol", "optgroup", "option", "p", "param", "search", "section",
    "summary", "table", "tbody", "td", "tfoot", "th", "thead", "title", "tr", "track", "ul",
];

fn starts_with_ci(s: &[u8], p: &str) -> bool {
    s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(p.as_bytes())
}

fn contains_ci(hay: &[u8], needle: &str) -> bool {
    let n = needle.as_bytes();
    hay.len() >= n.len() && hay.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

/// Which HTML_SEQUENCES entry (0..7) opens on `line`, if any.
pub fn block_sequence(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    if b.first() != Some(&b'<') {
        return None;
    }
    for name in ["script", "pre", "style", "textarea"] {
        if starts_with_ci(&b[1..], name) {
            let rest = &line[1 + name.len()..];
            match rest.chars().next() {
                None => return Some(0),
                Some(c) if c == '>' || is_js_space(c) => return Some(0),
                _ => {}
            }
        }
    }
    if line.starts_with("<!--") {
        return Some(1);
    }
    if line.starts_with("<?") {
        return Some(2);
    }
    if b.len() > 2 && b[1] == b'!' && b[2].is_ascii_alphabetic() {
        return Some(3);
    }
    if line.starts_with("<![CDATA[") {
        return Some(4);
    }
    {
        let start = if b.get(1) == Some(&b'/') { 2 } else { 1 };
        let tb = &b[start..];
        // The alternation picks the first listed name that matches with a valid lookahead.
        for name in BLOCK_NAMES {
            if starts_with_ci(tb, name) {
                let rest = &line[start + name.len()..];
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
        0 => ["</script>", "</pre>", "</style>", "</textarea>"]
            .iter()
            .any(|n| contains_ci(b, n)),
        1 => line.contains("-->"),
        2 => line.contains("?>"),
        3 => line.contains('>'),
        4 => line.contains("]]>"),
        _ => line.is_empty(),
    }
}
