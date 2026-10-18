//! Ports of `mdurl` (parse/format/encode/decode), `punycode.js`, and markdown-it's
//! `normalizeLink`, `normalizeLinkText` and `validateLink`.
use crate::prelude::*;

use crate::utils::js_trim;

#[derive(Default)]
struct Url {
    protocol: Option<String>,
    slashes: bool,
    auth: Option<String>,
    port: Option<String>,
    hostname: String,
    hash: Option<String>,
    search: Option<String>,
    pathname: Option<String>,
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

fn is_host_part_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '+' || c == '_' || c == '-'
}

/// `hostnamePartPattern` after replacing non-ASCII UTF-16 units with 'x'.
fn is_valid_host_part(part: &str) -> bool {
    let mut len = 0;
    for c in part.chars() {
        if (c as u32) > 127 {
            len += c.len_utf16();
        } else if is_host_part_char(c) {
            len += 1;
        } else {
            return false;
        }
    }
    len <= 63
}

const NON_HOST: &[u8] = b"%/?;#'{}|\\^`<>\" \r\n\t";

fn parse(url: &str) -> Url {
    let mut u = Url::default();
    let mut rest: String = js_trim(url).to_string();

    let pb = rest.as_bytes();
    let mut i = 0;
    while i < pb.len() && (pb[i].is_ascii_alphanumeric() || matches!(pb[i], b'.' | b'+' | b'-')) {
        i += 1;
    }
    if i > 0 && i < pb.len() && pb[i] == b':' {
        u.protocol = Some(rest[..i + 1].to_string());
        rest = rest[i + 1..].to_string();
    }
    let proto = u.protocol.as_deref();
    let hostless = proto == Some("javascript:");
    let slashed_exact = matches!(proto, Some("http:" | "https:" | "ftp:" | "gopher:" | "file:"));

    let slashes = rest.starts_with("//");
    if slashes && !hostless {
        rest.drain(..2);
        u.slashes = true;
    }

    if !hostless && (slashes || (proto.is_some() && !slashed_exact)) {
        let rb = rest.as_bytes();
        let host_end = rb
            .iter()
            .position(|&c| matches!(c, b'/' | b'?' | b'#'))
            .unwrap_or(rb.len());
        if let Some(at) = rb[..host_end].iter().rposition(|&c| c == b'@') {
            u.auth = Some(rest[..at].to_string());
            rest.drain(..at + 1);
        }
        let rb = rest.as_bytes();
        let mut host_end = rb.iter().position(|c| NON_HOST.contains(c)).unwrap_or(rb.len());
        if host_end > 0 && rb[host_end - 1] == b':' {
            host_end -= 1;
        }
        let mut host: String = rest[..host_end].to_string();
        rest.drain(..host_end);

        // parseHost: portPattern = /:[0-9]*$/
        let hb = host.as_bytes();
        let mut j = hb.len();
        while j > 0 && hb[j - 1].is_ascii_digit() {
            j -= 1;
        }
        if j > 0 && hb[j - 1] == b':' {
            if j < hb.len() {
                u.port = Some(host[j..].to_string());
            }
            host.truncate(j - 1);
        }
        u.hostname = host;

        let hn = u.hostname.clone();
        let hb = hn.as_bytes();
        let ipv6 = !hb.is_empty() && hb[0] == b'[' && hb[hb.len() - 1] == b']';
        if !ipv6 {
            let parts: Vec<&str> = hn.split('.').collect();
            for (pi, part) in parts.iter().enumerate() {
                if part.is_empty() || is_valid_host_part(part) {
                    continue;
                }
                let mut valid: Vec<&str> = parts[..pi].to_vec();
                let mut not_host: Vec<&str> = parts[pi + 1..].to_vec();
                // hostnamePartStart = /^([+a-z0-9A-Z_-]{0,63})(.*)$/ where `.` excludes line terminators
                let pbs = part.as_bytes();
                let mut k = 0;
                while k < pbs.len() && k < 63 && is_host_part_char(pbs[k] as char) {
                    k += 1;
                }
                let tail = &part[k..];
                if !tail.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
                    valid.push(&part[..k]);
                    not_host.insert(0, tail);
                }
                if !not_host.is_empty() {
                    let mut r = not_host.join(".");
                    r.push_str(&rest);
                    rest = r;
                }
                u.hostname = valid.join(".");
                break;
            }
        }
        if utf16_len(&u.hostname) > 255 {
            u.hostname.clear();
        }
        if ipv6 {
            u.hostname = if u.hostname.len() >= 2 {
                u.hostname[1..u.hostname.len() - 1].to_string()
            } else {
                String::new()
            };
        }
    }

    if let Some(h) = rest.find('#') {
        u.hash = Some(rest[h..].to_string());
        rest.truncate(h);
    }
    if let Some(q) = rest.find('?') {
        u.search = Some(rest[q..].to_string());
        rest.truncate(q);
    }
    let lower_slashed = u.protocol.as_deref().map_or(false, |p| {
        matches!(
            p.to_ascii_lowercase().as_str(),
            "http:" | "https:" | "ftp:" | "gopher:" | "file:"
        )
    });
    if !rest.is_empty() {
        u.pathname = Some(rest);
    } else if lower_slashed && !u.hostname.is_empty() {
        u.pathname = Some(String::new());
    }
    u
}

fn format(u: &Url) -> String {
    let mut r = String::new();
    if let Some(p) = &u.protocol {
        r.push_str(p);
    }
    if u.slashes {
        r.push_str("//");
    }
    if let Some(a) = &u.auth {
        if !a.is_empty() {
            r.push_str(a);
            r.push('@');
        }
    }
    if u.hostname.contains(':') {
        r.push('[');
        r.push_str(&u.hostname);
        r.push(']');
    } else {
        r.push_str(&u.hostname);
    }
    if let Some(p) = &u.port {
        if !p.is_empty() {
            r.push(':');
            r.push_str(p);
        }
    }
    for s in [&u.pathname, &u.search, &u.hash].into_iter().flatten() {
        r.push_str(s);
    }
    r
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn encode_keep(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b";/?:@&=+$,-_.!~*'()#".contains(&c)
}

/// `mdurl.encode(str)` with default exclusions and keepEscaped.
pub fn encode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push_str(&s[i..i + 3]);
            i += 3;
            continue;
        }
        if c < 128 && encode_keep(c) {
            out.push(c as char);
        } else {
            out.push('%');
            out.push(HEX[(c >> 4) as usize] as char);
            out.push(HEX[(c & 15) as usize] as char);
        }
        i += 1;
    }
    out
}

fn hexval(c: u8) -> u32 {
    (c as char).to_digit(16).unwrap()
}

/// `mdurl.decode(str, exclude)`.
fn decode(s: &str, exclude: &[u8]) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let is_esc = |i: usize| i + 2 < b.len() + 0 && b[i] == b'%' && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit();
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        if !(i + 2 < b.len() && is_esc(i)) {
            i += 1;
            continue;
        }
        out.push_str(&s[last..i]);
        // collect the run of %XX
        let mut bytes: Vec<u8> = Vec::new();
        let mut raw: Vec<usize> = Vec::new();
        while i + 2 < b.len() && is_esc(i) {
            bytes.push((hexval(b[i + 1]) * 16 + hexval(b[i + 2])) as u8);
            raw.push(i);
            i += 3;
        }
        last = i;
        let l = bytes.len();
        let mut k = 0;
        while k < l {
            let b1 = bytes[k] as u32;
            if b1 < 0x80 {
                if exclude.contains(&(b1 as u8)) {
                    out.push('%');
                    out.push(HEX[(b1 >> 4) as usize] as char);
                    out.push(HEX[(b1 & 15) as usize] as char);
                } else {
                    out.push(b1 as u8 as char);
                }
                k += 1;
                continue;
            }
            if (b1 & 0xE0) == 0xC0 && k + 1 < l {
                let b2 = bytes[k + 1] as u32;
                if (b2 & 0xC0) == 0x80 {
                    let chr = ((b1 << 6) & 0x7C0) | (b2 & 0x3F);
                    if chr < 0x80 {
                        out.push_str("\u{FFFD}\u{FFFD}");
                    } else {
                        out.push(char::from_u32(chr).unwrap());
                    }
                    k += 2;
                    continue;
                }
            }
            if (b1 & 0xF0) == 0xE0 && k + 2 < l {
                let b2 = bytes[k + 1] as u32;
                let b3 = bytes[k + 2] as u32;
                if (b2 & 0xC0) == 0x80 && (b3 & 0xC0) == 0x80 {
                    let chr = ((b1 << 12) & 0xF000) | ((b2 << 6) & 0xFC0) | (b3 & 0x3F);
                    if chr < 0x800 || (0xD800..=0xDFFF).contains(&chr) {
                        out.push_str("\u{FFFD}\u{FFFD}\u{FFFD}");
                    } else {
                        out.push(char::from_u32(chr).unwrap());
                    }
                    k += 3;
                    continue;
                }
            }
            if (b1 & 0xF8) == 0xF0 && k + 3 < l {
                let b2 = bytes[k + 1] as u32;
                let b3 = bytes[k + 2] as u32;
                let b4 = bytes[k + 3] as u32;
                if (b2 & 0xC0) == 0x80 && (b3 & 0xC0) == 0x80 && (b4 & 0xC0) == 0x80 {
                    let chr = ((b1 << 18) & 0x1C0000) | ((b2 << 12) & 0x3F000) | ((b3 << 6) & 0xFC0) | (b4 & 0x3F);
                    if !(0x10000..=0x10FFFF).contains(&chr) {
                        out.push_str("\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
                    } else {
                        out.push(char::from_u32(chr).unwrap());
                    }
                    k += 4;
                    continue;
                }
            }
            out.push('\u{FFFD}');
            k += 1;
        }
        let _ = &raw;
    }
    out.push_str(&s[last..]);
    out
}

// ---- punycode ----

const BASE: u32 = 36;
const TMIN: u32 = 1;
const TMAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const MAXINT: u32 = 0x7FFF_FFFF;

fn adapt(mut delta: u32, num_points: u32, first: bool) -> u32 {
    let mut k = 0;
    delta = if first { delta / DAMP } else { delta >> 1 };
    delta += delta / num_points;
    while delta > ((BASE - TMIN) * TMAX) >> 1 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (BASE - TMIN + 1) * delta / (delta + SKEW)
}

fn digit_to_basic(d: u32) -> char {
    (if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 }) as char
}

fn puny_encode(input: &str) -> Option<String> {
    let cps: Vec<u32> = input.chars().map(|c| c as u32).collect();
    let mut out = String::new();
    for &c in &cps {
        if c < 0x80 {
            out.push(c as u8 as char);
        }
    }
    let basic = out.len() as u32;
    let mut handled = basic;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias) = (128u32, 0u32, 72u32);
    let total = cps.len() as u32;
    while handled < total {
        let m = cps.iter().copied().filter(|&c| c >= n).min().unwrap_or(MAXINT);
        let hp1 = handled + 1;
        if m - n > (MAXINT - delta) / hp1 {
            return None;
        }
        delta += (m - n) * hp1;
        n = m;
        for &c in &cps {
            if c < n {
                delta += 1;
                if delta > MAXINT {
                    return None;
                }
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias { TMIN } else if k >= bias + TMAX { TMAX } else { k - bias };
                    if q < t {
                        break;
                    }
                    out.push(digit_to_basic(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit_to_basic(q));
                bias = adapt(delta, hp1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(out)
}

fn puny_decode(input: &str) -> Option<String> {
    let ib: Vec<u16> = input.encode_utf16().collect();
    let mut output: Vec<u32> = Vec::new();
    let basic = ib.iter().rposition(|&c| c == b'-' as u16).unwrap_or(0);
    for &c in &ib[..basic] {
        if c >= 0x80 {
            return None;
        }
        output.push(c as u32);
    }
    let (mut i, mut n, mut bias) = (0u32, 128u32, 72u32);
    let mut index = if basic > 0 { basic + 1 } else { 0 };
    while index < ib.len() {
        let oldi = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            if index >= ib.len() {
                return None;
            }
            let cp = ib[index] as u32;
            index += 1;
            let digit = match cp {
                0x30..=0x39 => 26 + (cp - 0x30),
                0x41..=0x5A => cp - 0x41,
                0x61..=0x7A => cp - 0x61,
                _ => BASE,
            };
            if digit >= BASE {
                return None;
            }
            if digit > (MAXINT - i) / w {
                return None;
            }
            i += digit * w;
            let t = if k <= bias { TMIN } else if k >= bias + TMAX { TMAX } else { k - bias };
            if digit < t {
                break;
            }
            if w > MAXINT / (BASE - t) {
                return None;
            }
            w *= BASE - t;
            k += BASE;
        }
        let out = output.len() as u32 + 1;
        bias = adapt(i - oldi, out, oldi == 0);
        if i / out > MAXINT - n {
            return None;
        }
        n += i / out;
        i %= out;
        output.insert(i as usize, n);
        i += 1;
    }
    // String.fromCodePoint throws above 0x10FFFF
    let mut s = String::new();
    for c in output {
        if c > 0x10FFFF {
            return None;
        }
        s.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
    }
    Some(s)
}

fn map_domain(domain: &str, f: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut result = String::new();
    let parts: Vec<&str> = domain.split('@').collect();
    let mut dom = domain;
    if parts.len() > 1 {
        result.push_str(parts[0]);
        result.push('@');
        dom = parts[1];
    }
    let dom = dom.replace(['\u{3002}', '\u{FF0E}', '\u{FF61}'], ".");
    let mut first = true;
    for label in dom.split('.') {
        if !first {
            result.push('.');
        }
        first = false;
        result.push_str(&f(label)?);
    }
    Some(result)
}

fn to_ascii(s: &str) -> Option<String> {
    map_domain(s, |l| {
        if l.is_ascii() {
            Some(l.to_string())
        } else {
            puny_encode(l).map(|e| ["xn--", &e].concat())
        }
    })
}

fn to_unicode(s: &str) -> Option<String> {
    map_domain(s, |l| {
        if let Some(rest) = l.strip_prefix("xn--") {
            puny_decode(&rest.to_lowercase())
        } else {
            Some(l.to_string())
        }
    })
}

fn recode_hostname(u: &Url) -> bool {
    !u.hostname.is_empty()
        && matches!(u.protocol.as_deref(), None | Some("http:" | "https:" | "mailto:"))
}

/// True when `normalize_link(url) == url`, so the common href skips `parse`,
/// `encode` and `format` and their allocations. The proof, from those three
/// functions: every byte is in `encode_keep`, so `encode` is the identity and
/// nothing is whitespace (`js_trim` is the identity), `%` or non-ASCII.
///
/// Without a protocol (the leading `[A-Za-z0-9.+-]` run is not followed by
/// `:`) and without `//` in front, `parse` skips the host branch and only cuts
/// the string into pathname, search and hash, which `format` concatenates back
/// in the original order.
///
/// With a protocol and `//`, `parse` takes the host branch. That is the
/// identity when the authority (up to the first `/`, `?` or `#`) is
/// `[auth@]host[:port]` with a non-empty auth, a port of one or more digits,
/// and a host of at most 255 bytes made of `.`-separated labels of at most 63
/// bytes of `[A-Za-z0-9+_-]`: `parseHost` then cuts off exactly the port,
/// every label passes `hostnamePartPattern` so nothing is moved into the path,
/// `to_ascii` and `encode` leave ASCII labels alone, and `format` writes
/// protocol, `//`, `auth@`, host, `:port` and the rest back in source order.
fn is_normalized(url: &str) -> bool {
    let b = url.as_bytes();
    if !b.iter().all(|&c| encode_keep(c)) || b.starts_with(b"//") {
        return false;
    }
    let n = b.iter().take_while(|&&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'+' | b'-')).count();
    if !(n > 0 && b.get(n) == Some(&b':')) {
        return true;
    }
    let Some(rest) = b[n + 1..].strip_prefix(b"//") else { return false };
    let end = rest.iter().position(|&c| matches!(c, b'/' | b'?' | b'#')).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host = match authority.iter().rposition(|&c| c == b'@') {
        Some(0) => return false,
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    let host = match host.iter().position(|&c| c == b':') {
        Some(i) => {
            let port = &host[i + 1..];
            if port.is_empty() || !port.iter().all(u8::is_ascii_digit) {
                return false;
            }
            &host[..i]
        }
        None => host,
    };
    host.len() <= 255
        && host.split(|&c| c == b'.').all(|label| {
            label.len() <= 63
                && label.iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'_' | b'-'))
        })
}

pub fn normalize_link(url: String) -> String {
    if is_normalized(&url) {
        return url;
    }
    normalize_link_slow(&url)
}

fn normalize_link_slow(url: &str) -> String {
    let mut u = parse(url);
    if recode_hostname(&u) {
        if let Some(h) = to_ascii(&u.hostname) {
            u.hostname = h;
        }
    }
    if let Some(a) = &u.auth {
        if !a.is_empty() {
            u.auth = Some(encode(a));
        }
    }
    if !u.hostname.is_empty() {
        u.hostname = encode(&u.hostname);
    }
    for f in [&mut u.pathname, &mut u.search, &mut u.hash] {
        if let Some(p) = f {
            if !p.is_empty() {
                *p = encode(p);
            }
        }
    }
    format(&u)
}

pub fn normalize_link_text(url: &str) -> String {
    let mut u = parse(url);
    if recode_hostname(&u) {
        if let Some(h) = to_unicode(&u.hostname) {
            u.hostname = h;
        }
    }
    decode(&format(&u), b";/?:@&=+$,#%")
}

pub fn validate_link(url: &str) -> bool {
    let t = js_trim(url);
    let b = t.as_bytes();
    let starts = |p: &str| b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p.as_bytes());
    if starts("vbscript:") || starts("javascript:") || starts("file:") || starts("data:") {
        return starts("data:image/gif;")
            || starts("data:image/png;")
            || starts("data:image/jpeg;")
            || starts("data:image/webp;");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_matches_full_normalization() {
        let edge = [
            "", "//a", "/a", "a:b", "a_b:c", "#x", "?x#y", "#a?b", "ch03-02-data-types.html",
            "mailto:x", "%41", "a%zz", "a b", ".:x", "+:", ":a", "a/b:c", "http://a.b/c", "a//b",
            "\u{e9}", "a\\b", "x?", "x#", " x", "x ",
            "https://github.com/a/b", "https://u:p@h.io:8080/p?q=1#f", "https://@a/b", "https://a:/b",
            "https://a:1:2/c", "https://a:x/", "http://a!b.c/d", "https://a;b/c", "https://a'b@c/",
            "https://a@b@c/d", "https:///x", "https://", "https://:80", "HTTP://A.B/C", "https://a..b/",
            "mailto:a@b.c", "javascript://a@b", "javascript://a:/b", "http:foo", "git+ssh://a_b.c/d",
            "https://a.b?x#y/z", "https://a.b#x?y", "https://a/b:c@d",
        ];
        let label = |n: usize| ["https://", &"a".repeat(n), ".b/c"].concat();
        for s in [label(63), label(64), label(255), label(300)] {
            assert_eq!(normalize_link(s.clone()), normalize_link_slow(&s), "{s:?}");
        }
        let hosts = |n: usize| ["https://", &["ab."].concat().repeat(n), "c/"].concat();
        for s in [hosts(84), hosts(85), hosts(86)] {
            assert_eq!(normalize_link(s.clone()), normalize_link_slow(&s), "{s:?}");
        }
        for s in edge {
            assert_eq!(normalize_link(s.to_string()), normalize_link_slow(s), "{s:?}");
        }
        let alphabet: Vec<u8> = (0u8..128)
            .filter(|&c| encode_keep(c))
            .chain(b"% \\\t\n".iter().copied())
            .collect();
        let mut seed: u32 = 0x9E37_79B9;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 8
        };
        for i in 0..4000 {
            let len = (next() % 24) as usize;
            let mut s = String::from(["", "https://", "http://u@", "mailto:", "x://a:1"][i % 5]);
            for _ in 0..len {
                let r = next();
                if r % 29 == 0 {
                    s.push('\u{e9}');
                } else {
                    s.push(alphabet[r as usize % alphabet.len()] as char);
                }
            }
            assert_eq!(normalize_link(s.clone()), normalize_link_slow(&s), "{s:?}");
        }
    }
}
