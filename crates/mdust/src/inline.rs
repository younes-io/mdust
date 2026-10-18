//! Inline tokenizer, a port of markdown-it's ParserInline, its rules and post-processing.
use crate::prelude::*;

use crate::block::Reference;
use crate::html::open_close_tag;
use crate::links::{parse_link_destination, parse_link_title};
use crate::url::{normalize_link, normalize_link_text, validate_link};
use crate::utils::{
    inline_entity, is_md_ascii_punct, is_punct_char, is_space, is_white_space, normalize_reference,
};
use crate::utils::FxMap;

const MAX_NESTING: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IKind {
    Text,
    TextSpecial,
    Softbreak,
    Hardbreak,
    CodeInline,
    HtmlInline,
    EmOpen,
    EmClose,
    StrongOpen,
    StrongClose,
    SOpen,
    SClose,
    LinkOpen,
    LinkClose,
    Image,
}

/// Ranges index `src` when `in_src`, otherwise the arena. Link and image
/// attributes always live in the arena.
#[derive(Clone, Copy)]
pub struct IToken {
    pub kind: IKind,
    pub in_src: bool,
    pub a: u32,
    pub b: u32,
    /// Link or image title.
    pub t: (u32, u32),
    /// Image alt as seen at depth 1 (text_special kept) and deeper (dropped).
    pub alt1: (u32, u32),
    pub alt2: (u32, u32),
}

impl IToken {
    fn new(kind: IKind) -> Self {
        IToken { kind, in_src: true, a: 0, b: 0, t: (0, 0), alt1: (0, 0), alt2: (0, 0) }
    }
}

#[derive(Clone, Copy)]
struct Delim {
    marker: u8,
    length: u32,
    token: u32,
    end: i32,
    open: bool,
    close: bool,
}

pub type Refs = FxMap<String, Reference>;

/// Buffers reused across inline parses of one document.
#[derive(Default)]
pub struct Scratch {
    tokens: Vec<IToken>,
    arena: String,
    lists: Vec<Vec<Delim>>,
    /// skip_token memo, indexed by position: `gen << 32 | next_pos`, valid while
    /// the stamp equals the current block's `gen`. Sized once to the longest block.
    cache: Vec<u64>,
    gen: u32,
    prev: Vec<usize>,
    backticks: Vec<u32>,
}

pub struct Inline<'a> {
    pub src: &'a str,
    s: &'a [u8],
    refs: &'a Refs,
    pos: usize,
    pos_max: usize,
    level: u32,
    pend_a: usize,
    pend_b: usize,
    pub tokens: Vec<IToken>,
    pub arena: String,
    lists: Vec<Vec<Delim>>,
    cur: usize,
    prev: Vec<usize>,
    cache: Vec<u64>,
    gen: u32,
    /// Last backtick run start per run length, 0 = none. Empty until the
    /// first backtick rule call for this block.
    backticks: Vec<u32>,
    skip_html_special: bool,
}

static TERMINATOR: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = is_terminator_slow(i as u8);
        i += 1;
    }
    t
};

#[inline(always)]
fn is_terminator(c: u8) -> bool {
    TERMINATOR[c as usize]
}

/// First index in `pos..max` holding a terminator byte, or `max`.
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
#[inline(always)]
fn find_terminator(s: &[u8], pos: usize, max: usize) -> usize {
    pos + s[pos..max].iter().position(|&c| is_terminator(c)).unwrap_or(max - pos)
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod simd_text {
    use super::is_terminator_slow;
    use core::arch::wasm32::*;

    // Nibble classifier: `c` is a terminator iff LOT[c & 15] & HIT[c >> 4] != 0.
    // Every terminator is ASCII with high nibble 0 or 2..7, so HIT gives those
    // seven nibbles one bit each and leaves the rest zero.
    static HIT: [u8; 16] = [1, 0, 2, 4, 8, 16, 32, 64, 0, 0, 0, 0, 0, 0, 0, 0];
    static LOT: [u8; 16] = {
        let mut t = [0u8; 16];
        let mut c = 0;
        while c < 128 {
            if is_terminator_slow(c as u8) {
                t[c & 15] |= HIT[c >> 4];
            }
            c += 1;
        }
        t
    };

    #[inline(always)]
    unsafe fn cls16(p: *const u8, lot: v128, hit: v128, m0f: v128) -> u32 {
        let v = v128_load(p as *const v128);
        let lo = i8x16_swizzle(lot, v128_and(v, m0f));
        let hi = i8x16_swizzle(hit, v128_and(u16x8_shr(v, 4), m0f));
        u8x16_bitmask(u8x16_ne(v128_and(lo, hi), u8x16_splat(0))) as u32
    }

    /// First index in `pos..max` holding a terminator byte, or `max`. Loads
    /// may cover bytes outside `pos..max` but never outside `s`.
    #[inline]
    pub fn find_terminator(s: &[u8], pos: usize, max: usize) -> usize {
        let mut i = pos;
        let p = s.as_ptr();
        let n = s.len();
        unsafe {
            let lot = v128_load(LOT.as_ptr() as *const v128);
            let hit = v128_load(HIT.as_ptr() as *const v128);
            let m0f = u8x16_splat(0x0f);
            while i < max && i + 16 <= n {
                let m = cls16(p.add(i), lot, hit, m0f);
                if m != 0 {
                    return (i + m.trailing_zeros() as usize).min(max);
                }
                i += 16;
            }
            if i < max {
                if n >= 16 {
                    let off = n - 16;
                    let m = cls16(p.add(off), lot, hit, m0f) >> (i - off);
                    return if m != 0 { (i + m.trailing_zeros() as usize).min(max) } else { max };
                }
                while i < max && !is_terminator_slow(s[i]) {
                    i += 1;
                }
            }
        }
        i
    }
}
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use simd_text::find_terminator;

/// markdown-it also stops text at `# $ % + - : = @ ^ { }`, but no rule in this
/// preset starts on them, so it appends each to pending one byte at a time;
/// scanning through them yields the same pending run. `]` stays a stop so
/// `parse_link_label` sees it at `pos`, and `>` stays a stop to match.
const fn is_terminator_slow(c: u8) -> bool {
    matches!(
        c,
        b'\n' | b'!' | b'&' | b'*' | b'<' | b'>' | b'[' | b'\\' | b']' | b'_' | b'`' | b'~'
    )
}

impl<'a> Inline<'a> {
    pub fn parse(src: &'a str, refs: &'a Refs) -> Self {
        Self::parse_with(src, refs, Scratch::default())
    }

    /// Takes `scratch` by value so its buffers move in and out without the
    /// default-and-drop churn of `mem::take` per inline block.
    pub fn parse_with(src: &'a str, refs: &'a Refs, scratch: Scratch) -> Self {
        let Scratch { mut tokens, mut arena, mut lists, mut cache, gen, mut prev, mut backticks } =
            scratch;
        tokens.clear();
        arena.clear();
        lists.truncate(1);
        match lists.first_mut() {
            Some(l) => l.clear(),
            None => lists.push(Vec::new()),
        }
        let gen = gen.wrapping_add(1);
        if gen == 0 {
            cache.clear();
        }
        let gen = gen.max(1);
        prev.clear();
        backticks.clear();
        let mut st = Inline {
            src,
            s: src.as_bytes(),
            refs,
            pos: 0,
            pos_max: src.len(),
            level: 0,
            pend_a: 0,
            pend_b: 0,
            tokens,
            arena,
            lists,
            cur: 0,
            prev,
            cache,
            gen,
            backticks,
            skip_html_special: false,
        };
        st.tokenize();
        st.post_process();
        st
    }

    pub fn recycle(self) -> Scratch {
        Scratch {
            tokens: self.tokens,
            arena: self.arena,
            lists: self.lists,
            cache: self.cache,
            gen: self.gen,
            prev: self.prev,
            backticks: self.backticks,
        }
    }

    #[inline]
    pub fn content(&self, t: &IToken) -> &str {
        if t.in_src {
            &self.src[t.a as usize..t.b as usize]
        } else {
            &self.arena[t.a as usize..t.b as usize]
        }
    }

    #[inline]
    pub fn arena_str(&self, r: (u32, u32)) -> &str {
        &self.arena[r.0 as usize..r.1 as usize]
    }

    fn arena_push(&mut self, s: &str) -> (u32, u32) {
        let a = self.arena.len() as u32;
        self.arena.push_str(s);
        (a, self.arena.len() as u32)
    }

    #[inline]
    fn ch(&self, pos: usize) -> u8 {
        if pos < self.s.len() {
            self.s[pos]
        } else {
            0
        }
    }

    fn append_pending(&mut self, a: usize, b: usize) {
        debug_assert!(self.pend_a == self.pend_b || self.pend_b == a);
        if self.pend_a == self.pend_b {
            self.pend_a = a;
        }
        self.pend_b = b;
    }

    fn push_pending(&mut self) {
        let mut t = IToken::new(IKind::Text);
        t.a = self.pend_a as u32;
        t.b = self.pend_b as u32;
        self.tokens.push(t);
        self.pend_a = 0;
        self.pend_b = 0;
    }

    fn push(&mut self, t: IToken) -> usize {
        if self.pend_a != self.pend_b {
            self.push_pending();
        }
        match t.kind {
            IKind::LinkClose => {
                self.level -= 1;
                self.cur = self.prev.pop().unwrap();
            }
            IKind::LinkOpen => {
                self.level += 1;
                self.prev.push(self.cur);
                self.lists.push(Vec::new());
                self.cur = self.lists.len() - 1;
            }
            _ => {}
        }
        self.tokens.push(t);
        self.tokens.len() - 1
    }

    fn push_src(&mut self, kind: IKind, a: usize, b: usize) -> usize {
        let mut t = IToken::new(kind);
        t.a = a as u32;
        t.b = b as u32;
        self.push(t)
    }

    fn push_arena(&mut self, kind: IKind, s: &str) -> usize {
        let (a, b) = self.arena_push(s);
        let mut t = IToken::new(kind);
        t.in_src = false;
        t.a = a;
        t.b = b;
        self.push(t)
    }

    fn tokenize(&mut self) {
        let end = self.pos_max;
        while self.pos < end {
            let ok = self.level < MAX_NESTING && self.run_rules(false);
            if ok {
                if self.pos >= end {
                    break;
                }
                continue;
            }
            let p = self.pos;
            self.pos += 1;
            self.append_pending(p, p + 1);
        }
        if self.pend_a != self.pend_b {
            self.push_pending();
        }
    }

    fn run_rules(&mut self, silent: bool) -> bool {
        let c = self.s[self.pos];
        if !is_terminator(c) {
            return self.rule_text(silent);
        }
        match c {
            b'\n' => self.rule_newline(silent),
            b'\\' => self.rule_escape(silent),
            b'`' => self.rule_backticks(silent),
            b'~' => !silent && self.rule_strikethrough(),
            b'*' | b'_' => !silent && self.rule_emphasis(),
            b'[' => self.rule_link(silent),
            b'!' => self.rule_image(silent),
            b'<' => self.rule_autolink(silent) || self.rule_html_inline(silent),
            b'&' => self.rule_entity(silent),
            _ => false,
        }
    }

    fn skip_token(&mut self) {
        let pos = self.pos;
        if self.cache.len() <= pos {
            self.cache.resize(self.pos_max + 1, 0);
        }
        let e = self.cache[pos];
        if (e >> 32) as u32 == self.gen {
            self.pos = e as u32 as usize;
            return;
        }
        let mut ok = false;
        if self.level < MAX_NESTING {
            self.level += 1;
            ok = self.run_rules(true);
            self.level -= 1;
        } else {
            self.pos = self.pos_max;
        }
        if !ok {
            self.pos += 1;
        }
        self.cache[pos] = (self.gen as u64) << 32 | self.pos as u64;
    }

    fn parse_link_label(&mut self, start: usize, disable_nested: bool) -> Option<usize> {
        let max = self.pos_max;
        let old_pos = self.pos;
        self.pos = start + 1;
        let mut level = 1;
        let mut found = false;
        while self.pos < max {
            let marker = self.s[self.pos];
            if marker == b']' {
                level -= 1;
                if level == 0 {
                    found = true;
                    break;
                }
            }
            let prev_pos = self.pos;
            self.skip_token();
            if marker == b'[' {
                if prev_pos == self.pos - 1 {
                    level += 1;
                } else if disable_nested {
                    self.pos = old_pos;
                    return None;
                }
            }
        }
        let r = if found { Some(self.pos) } else { None };
        self.pos = old_pos;
        r
    }

    // ---- rules ----

    fn rule_text(&mut self, silent: bool) -> bool {
        let mut pos = self.pos;
        let max = self.pos_max;
        pos = find_terminator(self.s, pos, max);
        if pos == self.pos {
            return false;
        }
        if !silent {
            self.append_pending(self.pos, pos);
        }
        self.pos = pos;
        true
    }

    fn rule_newline(&mut self, silent: bool) -> bool {
        let mut pos = self.pos;
        let max = self.pos_max;
        if !silent {
            let (pa, mut pb) = (self.pend_a, self.pend_b);
            if pb > pa && self.s[pb - 1] == b' ' {
                if pb - pa >= 2 && self.s[pb - 2] == b' ' {
                    pb -= 2;
                    while pb > pa && self.s[pb - 1] == b' ' {
                        pb -= 1;
                    }
                    self.pend_b = pb;
                    self.push(IToken::new(IKind::Hardbreak));
                } else {
                    self.pend_b = pb - 1;
                    self.push(IToken::new(IKind::Softbreak));
                }
            } else {
                self.push(IToken::new(IKind::Softbreak));
            }
        }
        pos += 1;
        while pos < max && is_space(self.s[pos]) {
            pos += 1;
        }
        self.pos = pos;
        true
    }

    fn rule_escape(&mut self, silent: bool) -> bool {
        let mut pos = self.pos + 1;
        let max = self.pos_max;
        if pos >= max {
            return false;
        }
        let ch1 = self.s[pos];
        if ch1 == b'\n' {
            if !silent {
                self.push(IToken::new(IKind::Hardbreak));
            }
            pos += 1;
            while pos < max && is_space(self.s[pos]) {
                pos += 1;
            }
            self.pos = pos;
            return true;
        }
        if ch1 == b' ' {
            if !silent {
                self.push_src(IKind::TextSpecial, pos - 1, pos);
            }
            self.pos = pos;
            return true;
        }
        let clen = self.src[pos..].chars().next().map_or(1, char::len_utf8);
        if !silent {
            if is_md_ascii_punct(ch1 as u32) {
                self.push_src(IKind::TextSpecial, pos, pos + clen);
            } else {
                self.push_src(IKind::TextSpecial, pos - 1, pos + clen);
            }
        }
        self.pos = pos + clen;
        true
    }

    fn rule_backticks(&mut self, silent: bool) -> bool {
        let start = self.pos;
        let max = self.pos_max;
        let s = self.s;
        let mut pos = start + 1;
        while pos < max && s[pos] == b'`' {
            pos += 1;
        }
        let opener_len = pos - start;
        if self.backticks.is_empty() {
            self.backticks.push(0);
            let mut p = 0;
            while let Some(i) = memchr(b'`', &s[p..]) {
                let st = p + i;
                p = st + 1;
                while p < s.len() && s[p] == b'`' {
                    p += 1;
                }
                let len = p - st;
                if self.backticks.len() <= len {
                    self.backticks.resize(len + 1, 0);
                }
                // st + 1 so a run at offset 0 is distinguishable from "none".
                self.backticks[len] = st as u32 + 1;
            }
        }
        let last = self.backticks.get(opener_len).copied().unwrap_or(0);
        if last > 0 && last as usize - 1 >= pos {
            let mut match_end = pos;
            while let Some(i) = memchr(b'`', &s[match_end..]) {
                let match_start = match_end + i;
                if match_start >= max {
                    break;
                }
                match_end = match_start + 1;
                while match_end < s.len() && s[match_end] == b'`' {
                    match_end += 1;
                }
                if match_end > max {
                    break;
                }
                if match_end - match_start == opener_len {
                    if !silent {
                        // Newlines become spaces, so the strip test treats them as spaces.
                        let sp = |c: u8| c == b' ' || c == b'\n';
                        let raw = &s[pos..match_start];
                        let (mut a, mut b) = (pos, match_start);
                        if raw.len() >= 2 && sp(raw[0]) && sp(raw[raw.len() - 1]) && raw.iter().any(|&c| !sp(c)) {
                            a += 1;
                            b -= 1;
                        }
                        if s[a..b].contains(&b'\n') {
                            let aa = self.arena.len();
                            for c in self.src[a..b].chars() {
                                self.arena.push(if c == '\n' { ' ' } else { c });
                            }
                            let mut t = IToken::new(IKind::CodeInline);
                            t.in_src = false;
                            t.a = aa as u32;
                            t.b = self.arena.len() as u32;
                            self.push(t);
                        } else {
                            self.push_src(IKind::CodeInline, a, b);
                        }
                    }
                    self.pos = match_end;
                    return true;
                }
            }
        }
        if !silent {
            self.append_pending(start, pos);
        }
        self.pos = pos;
        true
    }

    fn scan_delims(&self, start: usize, can_split_word: bool) -> (bool, bool, usize) {
        let max = self.pos_max;
        let marker = self.s[start];
        let last_char = if start == 0 {
            0x20
        } else {
            self.src[..start].chars().next_back().unwrap() as u32
        };
        let mut pos = start;
        while pos < max && self.s[pos] == marker {
            pos += 1;
        }
        let count = pos - start;
        let next_char = if pos < max { self.src[pos..].chars().next().unwrap() as u32 } else { 0x20 };
        let last_punct = is_md_ascii_punct(last_char) || is_punct_char(last_char);
        let next_punct = is_md_ascii_punct(next_char) || is_punct_char(next_char);
        let last_ws = is_white_space(last_char);
        let next_ws = is_white_space(next_char);
        let left = !next_ws && (!next_punct || last_ws || last_punct);
        let right = !last_ws && (!last_punct || next_ws || next_punct);
        let can_open = left && (can_split_word || !right || last_punct);
        let can_close = right && (can_split_word || !left || next_punct);
        (can_open, can_close, count)
    }

    fn rule_strikethrough(&mut self) -> bool {
        let start = self.pos;
        let (open, close, length) = self.scan_delims(start, true);
        let mut len = length;
        if len < 2 {
            return false;
        }
        if len % 2 == 1 {
            self.push_src(IKind::Text, start, start + 1);
            len -= 1;
        }
        let mut i = 0;
        while i < len {
            let tok = self.push_src(IKind::Text, start, start + 2);
            let cur = self.cur;
            self.lists[cur].push(Delim { marker: b'~', length: 0, token: tok as u32, end: -1, open, close });
            i += 2;
        }
        self.pos += length;
        true
    }

    fn rule_emphasis(&mut self) -> bool {
        let start = self.pos;
        let marker = self.s[start];
        let (open, close, length) = self.scan_delims(start, marker == b'*');
        for i in 0..length {
            let tok = self.push_src(IKind::Text, start + i, start + i + 1);
            let cur = self.cur;
            self.lists[cur].push(Delim {
                marker,
                length: length as u32,
                token: tok as u32,
                end: -1,
                open,
                close,
            });
        }
        self.pos += length;
        true
    }

    fn skip_ws_nl(&self, mut pos: usize, max: usize) -> usize {
        while pos < max {
            let c = self.s[pos];
            if !is_space(c) && c != b'\n' {
                break;
            }
            pos += 1;
        }
        pos
    }

    /// Reference form shared by link and image. Returns (href, title, new pos).
    fn reference_target(&mut self, mut pos: usize, label_start: usize, label_end: usize) -> Option<(String, String, usize)> {
        if self.refs.is_empty() {
            return None;
        }
        let max = self.pos_max;
        let mut label: Option<(usize, usize)> = None;
        if pos < max && self.s[pos] == b'[' {
            let start = pos + 1;
            match self.parse_link_label(pos, false) {
                Some(p) => {
                    label = Some((start, p));
                    pos = p + 1;
                }
                None => pos = label_end + 1,
            }
        } else {
            pos = label_end + 1;
        }
        let (la, lb) = match label {
            Some((a, b)) if b > a => (a, b),
            _ => (label_start, label_end),
        };
        let key = normalize_reference(&self.src[la..lb]);
        let r = self.refs.get(&key)?;
        Some((r.href.clone(), r.title.clone(), pos))
    }

    fn rule_link(&mut self, silent: bool) -> bool {
        let old_pos = self.pos;
        let max = self.pos_max;
        let label_start = self.pos + 1;
        let Some(label_end) = self.parse_link_label(self.pos, true) else { return false };
        let mut pos = label_end + 1;
        let mut parse_reference = true;
        let mut href = Cow::Borrowed("");
        let mut title = String::new();
        if pos < max && self.s[pos] == b'(' {
            parse_reference = false;
            pos = self.skip_ws_nl(pos + 1, max);
            if pos >= max {
                return false;
            }
            if let Some(res) = parse_link_destination(self.src, pos, self.pos_max) {
                href = normalize_link(res.str);
                if validate_link(&href) {
                    pos = res.pos;
                } else {
                    href = Cow::Borrowed("");
                }
                let start = pos;
                pos = self.skip_ws_nl(pos, max);
                let tr = parse_link_title(self.src, pos, self.pos_max, None);
                if pos < max && start != pos && tr.ok {
                    title = tr.str;
                    pos = self.skip_ws_nl(tr.pos, max);
                }
            }
            if pos < max && self.s[pos] == b')' {
                pos += 1;
            } else {
                parse_reference = true;
                // markdown-it does `pos++` here in UTF-16 units before it looks for a
                // reference label. Inside a 4-byte char that lands mid-surrogate, which
                // is never `[`, so label_end + 1 (the `(`) is an equivalent non-`[` spot.
                pos = match self.ch(pos) {
                    0..=0x7F => pos + 1,
                    0xC0..=0xDF => pos + 2,
                    0xE0..=0xEF => pos + 3,
                    _ => label_end + 1,
                };
            }
        }
        if parse_reference {
            match self.reference_target(pos, label_start, label_end) {
                Some((h, t, p)) => {
                    href = Cow::Owned(h);
                    title = t;
                    pos = p;
                }
                None => {
                    self.pos = old_pos;
                    return false;
                }
            }
        }
        if !silent {
            self.pos = label_start;
            self.pos_max = label_end;
            let mut t = IToken::new(IKind::LinkOpen);
            t.in_src = false;
            (t.a, t.b) = self.arena_push(&href);
            t.t = self.arena_push(&title);
            self.push(t);
            self.tokenize();
            self.push(IToken::new(IKind::LinkClose));
        }
        self.pos = pos;
        self.pos_max = max;
        true
    }

    fn rule_image(&mut self, silent: bool) -> bool {
        let old_pos = self.pos;
        let max = self.pos_max;
        if self.ch(self.pos + 1) != b'[' {
            return false;
        }
        let label_start = self.pos + 2;
        let Some(label_end) = self.parse_link_label(self.pos + 1, false) else { return false };
        let mut pos = label_end + 1;
        let mut href = Cow::Borrowed("");
        let title;
        if pos < max && self.s[pos] == b'(' {
            pos = self.skip_ws_nl(pos + 1, max);
            if pos >= max {
                return false;
            }
            if let Some(res) = parse_link_destination(self.src, pos, self.pos_max) {
                href = normalize_link(res.str);
                if validate_link(&href) {
                    pos = res.pos;
                } else {
                    href = Cow::Borrowed("");
                }
            }
            let start = pos;
            pos = self.skip_ws_nl(pos, max);
            let tr = parse_link_title(self.src, pos, self.pos_max, None);
            if pos < max && start != pos && tr.ok {
                title = tr.str;
                pos = self.skip_ws_nl(tr.pos, max);
            } else {
                title = String::new();
            }
            if pos >= max || self.s[pos] != b')' {
                self.pos = old_pos;
                return false;
            }
            pos += 1;
        } else {
            match self.reference_target(pos, label_start, label_end) {
                Some((h, t, p)) => {
                    href = Cow::Owned(h);
                    title = t;
                    pos = p;
                }
                None => {
                    self.pos = old_pos;
                    return false;
                }
            }
        }
        if !silent {
            let content = &self.src[label_start..label_end];
            let child = Inline::parse(content, self.refs);
            let mut alt1 = String::new();
            let mut alt2 = String::new();
            child.render_as_text(&mut alt1, true);
            child.render_as_text(&mut alt2, false);
            let mut t = IToken::new(IKind::Image);
            t.in_src = false;
            (t.a, t.b) = self.arena_push(&href);
            t.t = self.arena_push(&title);
            t.alt1 = self.arena_push(&alt1);
            t.alt2 = self.arena_push(&alt2);
            self.push(t);
        }
        self.pos = pos;
        self.pos_max = max;
        true
    }

    /// `renderInlineAsText`. text_special counts only at the top image level
    /// because markdown-it's join_alt converts it to text one level deep.
    pub fn render_as_text(&self, out: &mut String, keep_special: bool) {
        for t in &self.tokens {
            match t.kind {
                IKind::Text | IKind::CodeInline | IKind::HtmlInline => out.push_str(self.content(t)),
                IKind::TextSpecial if keep_special => out.push_str(self.content(t)),
                IKind::Image => out.push_str(self.arena_str(t.alt2)),
                IKind::Softbreak | IKind::Hardbreak => out.push('\n'),
                _ => {}
            }
        }
    }

    fn rule_autolink(&mut self, silent: bool) -> bool {
        let start = self.pos;
        let max = self.pos_max;
        let mut pos = start;
        loop {
            pos += 1;
            if pos >= max {
                return false;
            }
            let c = self.s[pos];
            if c == b'<' {
                return false;
            }
            if c == b'>' {
                break;
            }
        }
        let url = &self.src[start + 1..pos];
        let full = if is_autolink(url) {
            normalize_link(Cow::Borrowed(url))
        } else if is_email(url) {
            normalize_link(Cow::Owned(["mailto:", url].concat()))
        } else {
            return false;
        };
        if !validate_link(&full) {
            return false;
        }
        if !silent {
            let mut t = IToken::new(IKind::LinkOpen);
            t.in_src = false;
            (t.a, t.b) = self.arena_push(&full);
            t.t = (0, 0);
            self.push(t);
            let text = normalize_link_text(url);
            self.push_arena(IKind::Text, &text);
            self.push(IToken::new(IKind::LinkClose));
        }
        self.pos += url.len() + 2;
        true
    }

    fn scan_special(&mut self, pos: usize) -> Option<usize> {
        let src = self.src;
        let rest = &src[pos..];
        let (start, term) = if rest.starts_with("<!--") {
            if src[pos + 4..].starts_with('>') {
                return Some(pos + 5);
            }
            if src[pos + 4..].starts_with("->") {
                return Some(pos + 6);
            }
            (pos + 4, "-->")
        } else if rest.starts_with("<![CDATA[") {
            (pos + 9, "]]>")
        } else if self.ch(pos + 1) == b'?' {
            (pos + 2, "?>")
        } else if self.ch(pos + 2).is_ascii_alphabetic() {
            (pos + 3, ">")
        } else {
            return None;
        };
        if self.skip_html_special {
            return None;
        }
        match src[start..].find(term) {
            Some(i) => Some(start + i + term.len()),
            None => {
                self.skip_html_special = true;
                None
            }
        }
    }

    fn rule_html_inline(&mut self, silent: bool) -> bool {
        let max = self.pos_max;
        let pos = self.pos;
        if pos + 2 >= max {
            return false;
        }
        let ch = self.s[pos + 1];
        let end = if ch == b'!' || ch == b'?' {
            self.scan_special(pos)
        } else if ch == b'/' || ch.is_ascii_alphabetic() {
            open_close_tag(&self.src[pos..]).map(|l| pos + l)
        } else {
            return false;
        };
        let Some(end) = end else { return false };
        if !silent {
            self.push_src(IKind::HtmlInline, pos, end);
        }
        self.pos = end;
        true
    }

    fn rule_entity(&mut self, silent: bool) -> bool {
        let pos = self.pos;
        if pos + 1 >= self.pos_max {
            return false;
        }
        let a = self.arena.len();
        let mut tmp = core::mem::take(&mut self.arena);
        let r = inline_entity(&self.s[pos..], &mut tmp);
        self.arena = tmp;
        let Some(n) = r else { return false };
        if silent {
            self.arena.truncate(a);
        } else {
            let mut t = IToken::new(IKind::TextSpecial);
            t.in_src = false;
            t.a = a as u32;
            t.b = self.arena.len() as u32;
            self.push(t);
        }
        self.pos += n;
        true
    }

    // ---- post-processing ----

    fn post_process(&mut self) {
        for l in self.lists.iter_mut() {
            balance_pairs(l);
        }
        for i in 0..self.lists.len() {
            strikethrough_post(&mut self.tokens, &self.lists[i], self.src, &self.arena);
        }
        for i in 0..self.lists.len() {
            emphasis_post(&mut self.tokens, &self.lists[i]);
        }
    }
}

fn memchr(c: u8, h: &[u8]) -> Option<usize> {
    // Eight bytes per step; wasm32 has no SIMD here, so use the zero-byte trick on u64.
    let pat = u64::from_ne_bytes([c; 8]);
    let mut i = 0;
    while i + 8 <= h.len() {
        let w = u64::from_le_bytes(h[i..i + 8].try_into().unwrap()) ^ pat;
        let z = w.wrapping_sub(0x0101_0101_0101_0101) & !w & 0x8080_8080_8080_8080;
        if z != 0 {
            return Some(i + (z.trailing_zeros() / 8) as usize);
        }
        i += 8;
    }
    h[i..].iter().position(|&x| x == c).map(|k| i + k)
}

fn is_autolink(url: &str) -> bool {
    let b = url.as_bytes();
    let Some(colon) = b.iter().position(|&c| c == b':') else { return false };
    if !(2..=32).contains(&colon) || !b[0].is_ascii_alphabetic() {
        return false;
    }
    if !b[1..colon].iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'.' | b'-')) {
        return false;
    }
    b[colon + 1..].iter().all(|&c| c > 0x20 && c != b'<' && c != b'>')
}

fn is_email(url: &str) -> bool {
    let b = url.as_bytes();
    let Some(at) = b.iter().position(|&c| c == b'@') else { return false };
    if at == 0 {
        return false;
    }
    let local_ok = b[..at]
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&c));
    if !local_ok {
        return false;
    }
    let domain = &b[at + 1..];
    if domain.is_empty() {
        return false;
    }
    domain.split(|&c| c == b'.').all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && l[0].is_ascii_alphanumeric()
            && l[l.len() - 1].is_ascii_alphanumeric()
            && l.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'-')
    })
}

fn balance_pairs(d: &mut [Delim]) {
    let max = d.len();
    if max == 0 {
        return;
    }
    // openersBottom per marker: '*', '_', '~'
    let mut bottoms: [[i32; 6]; 3] = [[-1; 6]; 3];
    let mi = |m: u8| match m {
        b'*' => 0,
        b'_' => 1,
        _ => 2,
    };
    let mut header_idx = 0usize;
    let mut last_token_idx: i64 = -2;
    let mut jumps: Vec<i32> = Vec::with_capacity(max);
    for closer_idx in 0..max {
        jumps.push(0);
        let closer = d[closer_idx];
        if d[header_idx].marker != closer.marker || last_token_idx != closer.token as i64 - 1 {
            header_idx = closer_idx;
        }
        last_token_idx = closer.token as i64;
        if !closer.close {
            continue;
        }
        let m = mi(closer.marker);
        let bi = (if closer.open { 3 } else { 0 }) + (closer.length % 3) as usize;
        let min_opener_idx = bottoms[m][bi];
        let mut opener_idx = header_idx as i32 - jumps[header_idx] - 1;
        let mut new_min = opener_idx;
        while opener_idx > min_opener_idx {
            let oi = opener_idx as usize;
            let opener = d[oi];
            if opener.marker == closer.marker && opener.open && opener.end < 0 {
                let mut odd = false;
                if opener.close || d[closer_idx].open {
                    if (opener.length + closer.length) % 3 == 0
                        && (opener.length % 3 != 0 || closer.length % 3 != 0)
                    {
                        odd = true;
                    }
                }
                if !odd {
                    let last_jump = if oi > 0 && !d[oi - 1].open { jumps[oi - 1] + 1 } else { 0 };
                    jumps[closer_idx] = closer_idx as i32 - opener_idx + last_jump;
                    jumps[oi] = last_jump;
                    d[closer_idx].open = false;
                    d[oi].end = closer_idx as i32;
                    d[oi].close = false;
                    new_min = -1;
                    last_token_idx = -2;
                    break;
                }
            }
            opener_idx -= jumps[oi] + 1;
        }
        if new_min != -1 {
            let bi = (if d[closer_idx].open { 3 } else { 0 }) + (closer.length % 3) as usize;
            bottoms[m][bi] = new_min;
        }
    }
}

fn strikethrough_post(tokens: &mut [IToken], d: &[Delim], src: &str, arena: &str) {
    let mut lone: Vec<usize> = Vec::new();
    for sd in d {
        if sd.marker != b'~' || sd.end == -1 {
            continue;
        }
        let ed = &d[sd.end as usize];
        tokens[sd.token as usize].kind = IKind::SOpen;
        tokens[ed.token as usize].kind = IKind::SClose;
        let p = &tokens[ed.token as usize - 1];
        if p.kind == IKind::Text {
            let c = if p.in_src { &src[p.a as usize..p.b as usize] } else { &arena[p.a as usize..p.b as usize] };
            if c == "~" {
                lone.push(ed.token as usize - 1);
            }
        }
    }
    while let Some(i) = lone.pop() {
        let mut j = i + 1;
        while j < tokens.len() && tokens[j].kind == IKind::SClose {
            j += 1;
        }
        j -= 1;
        if i != j {
            tokens.swap(i, j);
        }
    }
}

fn emphasis_post(tokens: &mut [IToken], d: &[Delim]) {
    let mut i = d.len() as isize - 1;
    while i >= 0 {
        let sd = d[i as usize];
        if (sd.marker != b'_' && sd.marker != b'*') || sd.end == -1 {
            i -= 1;
            continue;
        }
        let ed = d[sd.end as usize];
        let iu = i as usize;
        let se = sd.end as usize;
        let is_strong = i > 0
            && d[iu - 1].end == sd.end + 1
            && d[iu - 1].marker == sd.marker
            && d[iu - 1].token + 1 == sd.token
            && d[se + 1].token == ed.token + 1;
        let (o, c) = if is_strong { (IKind::StrongOpen, IKind::StrongClose) } else { (IKind::EmOpen, IKind::EmClose) };
        tokens[sd.token as usize].kind = o;
        tokens[ed.token as usize].kind = c;
        if is_strong {
            let t1 = &mut tokens[d[iu - 1].token as usize];
            t1.b = t1.a;
            let t2 = &mut tokens[d[se + 1].token as usize];
            t2.b = t2.a;
            i -= 1;
        }
        i -= 1;
    }
}
