use crate::tables::{ENTITY_BLOB, ENTITY_INDEX, PUNCT_RANGES};

#[inline]
pub fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

pub fn is_white_space(c: u32) -> bool {
    if (0x2000..=0x200A).contains(&c) {
        return true;
    }
    matches!(
        c,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x202F | 0x205F | 0x3000
    )
}

#[inline]
pub fn is_md_ascii_punct(c: u32) -> bool {
    matches!(c, 0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E)
}

pub fn is_punct_char(c: u32) -> bool {
    if c < 0x80 {
        return is_md_ascii_punct(c);
    }
    let mut lo = 0usize;
    let mut hi = PUNCT_RANGES.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (a, b) = PUNCT_RANGES[mid];
        if c < a {
            hi = mid;
        } else if c > b {
            lo = mid + 1;
        } else {
            return true;
        }
    }
    false
}

/// JS `\s` / `String.prototype.trim` whitespace set.
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{0B}'
            | '\u{0C}'
            | '\r'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

pub fn ascii_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

static NEEDS_ESCAPE: [bool; 256] = {
    let mut t = [false; 256];
    t[b'&' as usize] = true;
    t[b'<' as usize] = true;
    t[b'>' as usize] = true;
    t[b'"' as usize] = true;
    t
};

const LO: u64 = 0x0101_0101_0101_0101;
const HI: u64 = 0x8080_8080_8080_8080;

/// High bit set in each byte of `w` that equals `c`.
#[inline(always)]
pub fn swar_eq(w: u64, c: u8) -> u64 {
    let x = w ^ (LO * c as u64);
    x.wrapping_sub(LO) & !x & HI
}

#[inline(always)]
fn word(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

/// Index of the first byte in `b[from..]` that is `&`, `<`, `>` or `"`.
#[inline]
fn next_escape(b: &[u8], mut i: usize) -> usize {
    while i + 8 <= b.len() {
        let w = word(b, i);
        let m = swar_eq(w, b'&') | swar_eq(w, b'<') | swar_eq(w, b'>') | swar_eq(w, b'"');
        if m != 0 {
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < b.len() && !NEEDS_ESCAPE[b[i] as usize] {
        i += 1;
    }
    i
}

pub fn escape_html_into(out: &mut String, s: &str) {
    let b = s.as_bytes();
    let mut last = 0;
    let mut i = next_escape(b, 0);
    while i < b.len() {
        let rep = match b[i] {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            _ => "&quot;",
        };
        out.push_str(&s[last..i]);
        out.push_str(rep);
        last = i + 1;
        i = next_escape(b, last);
    }
    out.push_str(&s[last..]);
}

/// Whether the input needs markdown-it's `\r` and NUL normalization.
pub fn needs_normalize(b: &[u8]) -> bool {
    let mut i = 0;
    while i + 8 <= b.len() {
        let w = word(b, i);
        if swar_eq(w, b'\r') | swar_eq(w, 0) != 0 {
            return true;
        }
        i += 8;
    }
    b[i..].iter().any(|&c| c == b'\r' || c == 0)
}

pub fn lookup_entity(name: &str) -> Option<&'static str> {
    let mut lo = 0usize;
    let mut hi = ENTITY_INDEX.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (off, nl, vl) = ENTITY_INDEX[mid];
        let off = off as usize;
        let n = &ENTITY_BLOB[off..off + nl as usize];
        match n.cmp(name) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                let v = off + nl as usize;
                return Some(&ENTITY_BLOB[v..v + vl as usize]);
            }
        }
    }
    None
}

pub fn is_valid_entity_code(c: u32) -> bool {
    if (0xD800..=0xDFFF).contains(&c) {
        return false;
    }
    if (0xFDD0..=0xFDEF).contains(&c) {
        return false;
    }
    if (c & 0xFFFF) == 0xFFFF || (c & 0xFFFF) == 0xFFFE {
        return false;
    }
    if c <= 0x08 || c == 0x0B || (0x0E..=0x1F).contains(&c) || (0x7F..=0x9F).contains(&c) {
        return false;
    }
    c <= 0x10FFFF
}

fn parse_num(digits: &str, hex: bool) -> u64 {
    let radix = if hex { 16 } else { 10 };
    let mut v: u64 = 0;
    for c in digits.bytes() {
        let d = (c as char).to_digit(radix).unwrap() as u64;
        v = (v * radix as u64 + d).min(0x7FFF_FFFF);
    }
    v
}

const C1: [u16; 32] = [
    8364, 0, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 0, 381, 0, 0, 8216,
    8217, 8220, 8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 0, 382, 376,
];

/// `entities.decodeHTMLStrict` applied to a whole `&...;` reference whose body is `name`.
fn decode_html_strict(name: &str, out: &mut String) -> bool {
    if let Some(num) = name.strip_prefix('#') {
        let (digits, hex) = match num.as_bytes().first() {
            Some(b'x') | Some(b'X') => (&num[1..], true),
            _ => (num, false),
        };
        let ok = !digits.is_empty()
            && digits
                .bytes()
                .all(|c| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() });
        if !ok {
            return false;
        }
        let mut cp = parse_num(digits, hex) as u32;
        if cp == 0 || (0xD800..=0xDFFF).contains(&cp) || cp > 0x10FFFF {
            cp = 0xFFFD;
        } else if (128..=159).contains(&cp) {
            let r = C1[(cp - 128) as usize];
            if r != 0 {
                cp = r as u32;
            }
        }
        out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
        return true;
    }
    match lookup_entity(name) {
        Some(v) => {
            out.push_str(v);
            true
        }
        None => false,
    }
}

/// Matches `&([a-z#][a-z0-9]{1,31});` (case-insensitive) at the start of `b`.
/// Returns the length of the body (between `&` and `;`).
pub fn match_entity_ref(b: &[u8]) -> Option<usize> {
    if b.len() < 3 || b[0] != b'&' {
        return None;
    }
    let first = b[1];
    if !(first.is_ascii_alphabetic() || first == b'#') {
        return None;
    }
    let mut i = 2;
    while i < b.len() && i < 34 && b[i].is_ascii_alphanumeric() {
        i += 1;
    }
    if i == 2 || i >= b.len() || b[i] != b';' {
        return None;
    }
    Some(i - 1)
}

pub fn push_code_point(out: &mut String, c: u32) {
    out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
}

fn replace_entity_pattern(name: &str, whole: &str, out: &mut String) {
    if let Some(num) = name.strip_prefix('#') {
        let (digits, hex) = match num.as_bytes().first() {
            Some(b'x') | Some(b'X') => (&num[1..], true),
            _ => (num, false),
        };
        let ok = !digits.is_empty()
            && digits.len() <= 8
            && digits
                .bytes()
                .all(|c| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() });
        if ok {
            let code = parse_num(digits, hex) as u32;
            if is_valid_entity_code(code) {
                push_code_point(out, code);
            } else {
                out.push_str(whole);
            }
            return;
        }
    }
    if !decode_html_strict(name, out) {
        out.push_str(whole);
    }
}

pub fn unescape_all_into(out: &mut String, s: &str) {
    let b = s.as_bytes();
    if !b.iter().any(|&c| c == b'\\' || c == b'&') {
        out.push_str(s);
        return;
    }
    let mut last = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\\' && i + 1 < b.len() && is_md_ascii_punct(b[i + 1] as u32) {
            out.push_str(&s[last..i]);
            out.push(b[i + 1] as char);
            i += 2;
            last = i;
            continue;
        }
        if c == b'&' {
            if let Some(n) = match_entity_ref(&b[i..]) {
                out.push_str(&s[last..i]);
                replace_entity_pattern(&s[i + 1..i + 1 + n], &s[i..i + n + 2], out);
                i += n + 2;
                last = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&s[last..]);
}

pub fn unescape_all(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    unescape_all_into(&mut out, s);
    out
}

/// Decoding for inline `&...;` entities (rules_inline/entity.ts). Returns (consumed bytes).
pub fn inline_entity(b: &[u8], out: &mut String) -> Option<usize> {
    // b[0] == '&'
    if b.len() < 2 {
        return None;
    }
    if b[1] == b'#' {
        // DIGITAL_RE = /^&#((?:x[a-f0-9]{1,6}|[0-9]{1,7}));/i
        let (start, hex, maxd) = if b.len() > 2 && (b[2] | 0x20) == b'x' {
            (3, true, 6)
        } else {
            (2, false, 7)
        };
        let mut i = start;
        while i < b.len()
            && i - start < maxd
            && (if hex { b[i].is_ascii_hexdigit() } else { b[i].is_ascii_digit() })
        {
            i += 1;
        }
        if i == start || i >= b.len() || b[i] != b';' {
            if hex {
                // retry decimal branch can't match since 'x' is not a digit
            }
            return None;
        }
        let digits = std::str::from_utf8(&b[start..i]).unwrap();
        let code = parse_num(digits, hex) as u32;
        if is_valid_entity_code(code) {
            push_code_point(out, code);
        } else {
            out.push('\u{FFFD}');
        }
        return Some(i + 1);
    }
    // NAMED_RE = /^&([a-z][a-z0-9]{1,31});/i
    if !b[1].is_ascii_alphabetic() {
        return None;
    }
    let n = match_entity_ref(b)?;
    let name = std::str::from_utf8(&b[1..1 + n]).unwrap();
    let v = lookup_entity(name)?;
    out.push_str(v);
    Some(n + 2)
}

/// `normalizeReference`: trim, collapse whitespace, then toLowerCase().toUpperCase().
pub fn normalize_reference(s: &str) -> String {
    let t = js_trim(s);
    let mut collapsed = String::with_capacity(t.len());
    let mut in_ws = false;
    for c in t.chars() {
        if is_js_space(c) {
            if !in_ws {
                collapsed.push(' ');
                in_ws = true;
            }
        } else {
            collapsed.push(c);
            in_ws = false;
        }
    }
    if collapsed.is_ascii() {
        collapsed.make_ascii_uppercase();
        return collapsed;
    }
    collapsed.to_lowercase().to_uppercase()
}

#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut v = 0u64;
            for (i, &b) in chunk.iter().enumerate() {
                v |= (b as u64) << (i * 8);
            }
            self.add(v);
        }
    }
    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

impl FxHasher {
    #[inline]
    fn add(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

pub type FxMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<FxHasher>>;
