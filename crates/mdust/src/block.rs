//! Block-level tokenizer, a port of markdown-it's ParserBlock and its rules.
use crate::prelude::*;

use crate::html;
use crate::links::{parse_link_destination, parse_link_title};
use crate::url::{normalize_link, validate_link};
use crate::utils::{ascii_trim, is_space, js_trim, normalize_reference};
use crate::utils::FxMap;

const MAX_NESTING: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    ParagraphOpen,
    ParagraphClose,
    HeadingOpen(u8),
    HeadingClose(u8),
    BlockquoteOpen,
    BlockquoteClose,
    BulletListOpen,
    BulletListClose,
    OrderedListOpen(u32),
    OrderedListClose,
    ListItemOpen,
    ListItemClose,
    TableOpen,
    TableClose,
    TheadOpen,
    TheadClose,
    TbodyOpen,
    TbodyClose,
    TrOpen,
    TrClose,
    ThOpen(u8),
    ThClose,
    TdOpen(u8),
    TdClose,
    Inline,
    CodeBlock,
    Fence,
    Hr,
    HtmlBlock,
}

impl Kind {
    pub fn nesting(self) -> i8 {
        use Kind::*;
        match self {
            ParagraphOpen | HeadingOpen(_) | BlockquoteOpen | BulletListOpen | OrderedListOpen(_)
            | ListItemOpen | TableOpen | TheadOpen | TbodyOpen | TrOpen | ThOpen(_) | TdOpen(_) => 1,
            ParagraphClose | HeadingClose(_) | BlockquoteClose | BulletListClose
            | OrderedListClose | ListItemClose | TableClose | TheadClose | TbodyClose | TrClose
            | ThClose | TdClose => -1,
            Inline | CodeBlock | Fence | Hr | HtmlBlock => 0,
        }
    }

    /// Whole tag for a token with no attributes: `<p>`, or `<ul>\n` when `lf`
    /// (closing tags always end in a newline).
    pub fn html(self, lf: bool) -> &'static str {
        use Kind::*;
        const HO: [&str; 7] = ["", "<h1>", "<h2>", "<h3>", "<h4>", "<h5>", "<h6>"];
        const HOL: [&str; 7] = ["", "<h1>\n", "<h2>\n", "<h3>\n", "<h4>\n", "<h5>\n", "<h6>\n"];
        const HC: [&str; 7] = ["", "</h1>\n", "</h2>\n", "</h3>\n", "</h4>\n", "</h5>\n", "</h6>\n"];
        match self {
            ParagraphOpen => if lf { "<p>\n" } else { "<p>" },
            ParagraphClose => "</p>\n",
            HeadingOpen(l) => if lf { HOL[l as usize] } else { HO[l as usize] },
            HeadingClose(l) => HC[l as usize],
            BlockquoteOpen => if lf { "<blockquote>\n" } else { "<blockquote>" },
            BlockquoteClose => "</blockquote>\n",
            BulletListOpen => if lf { "<ul>\n" } else { "<ul>" },
            BulletListClose => "</ul>\n",
            OrderedListOpen(_) => if lf { "<ol>\n" } else { "<ol>" },
            OrderedListClose => "</ol>\n",
            ListItemOpen => if lf { "<li>\n" } else { "<li>" },
            ListItemClose => "</li>\n",
            TableOpen => if lf { "<table>\n" } else { "<table>" },
            TableClose => "</table>\n",
            TheadOpen => if lf { "<thead>\n" } else { "<thead>" },
            TheadClose => "</thead>\n",
            TbodyOpen => if lf { "<tbody>\n" } else { "<tbody>" },
            TbodyClose => "</tbody>\n",
            TrOpen => if lf { "<tr>\n" } else { "<tr>" },
            TrClose => "</tr>\n",
            ThOpen(_) => if lf { "<th>\n" } else { "<th>" },
            ThClose => "</th>\n",
            TdOpen(_) => if lf { "<td>\n" } else { "<td>" },
            TdClose => "</td>\n",
            Hr => "<hr>\n",
            Inline | CodeBlock | Fence | HtmlBlock => "",
        }
    }

    pub fn tag(self) -> &'static str {
        use Kind::*;
        const H: [&str; 7] = ["", "h1", "h2", "h3", "h4", "h5", "h6"];
        match self {
            ParagraphOpen | ParagraphClose => "p",
            HeadingOpen(l) | HeadingClose(l) => H[l as usize],
            BlockquoteOpen | BlockquoteClose => "blockquote",
            BulletListOpen | BulletListClose => "ul",
            OrderedListOpen(_) | OrderedListClose => "ol",
            ListItemOpen | ListItemClose => "li",
            TableOpen | TableClose => "table",
            TheadOpen | TheadClose => "thead",
            TbodyOpen | TbodyClose => "tbody",
            TrOpen | TrClose => "tr",
            ThOpen(_) | ThClose => "th",
            TdOpen(_) | TdClose => "td",
            CodeBlock | Fence => "code",
            Hr => "hr",
            Inline | HtmlBlock => "",
        }
    }
}

/// Set in a span's start offset when the span indexes `Blocks::buf` instead of the
/// source. Most block content is a verbatim run of source lines, so it stays a span
/// into `src` and is never copied; only lines that need indent stripped or padding
/// added, plus synthesized text, go through the arena.
pub const ARENA: u32 = 1 << 31;

pub struct Token {
    pub kind: Kind,
    pub hidden: bool,
    pub level: u32,
    /// Content span, see `Blocks::slice`.
    pub c0: u32,
    pub c1: u32,
    /// Fence info span, see `Blocks::slice`.
    pub i0: u32,
    pub i1: u32,
}

pub struct Reference {
    pub href: String,
    pub title: String,
}

pub struct Blocks<'a> {
    pub src: &'a str,
    pub tokens: Vec<Token>,
    pub buf: String,
    pub refs: FxMap<String, Reference>,
}

impl<'a> Blocks<'a> {
    /// Resolves a span to its text in `src` or, when `a` carries `ARENA`, in `buf`.
    pub fn slice(&self, a: u32, b: u32) -> &str {
        if a & ARENA == 0 {
            &self.src[a as usize..b as usize]
        } else {
            &self.buf[(a ^ ARENA) as usize..b as usize]
        }
    }
    pub fn content(&self, t: &Token) -> &str {
        self.slice(t.c0, t.c1)
    }
    pub fn info(&self, t: &Token) -> &str {
        self.slice(t.i0, t.i1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parent {
    Root,
    Blockquote,
    List,
    Paragraph,
    Reference,
    Table,
}

#[derive(Clone, Copy)]
enum Chain {
    /// Terminators for paragraph, reference and lheading.
    Paragraph,
    Blockquote,
    List,
}

/// One source line. `start` is the first byte, `end` the newline (or EOF),
/// `t_shift` the bytes of leading whitespace, `s_count` that whitespace in
/// columns, `bs_count` the column where the parent block's marker ends.
#[derive(Clone, Copy)]
struct Line {
    start: usize,
    end: usize,
    t_shift: usize,
    s_count: i32,
    bs_count: i32,
}

struct State<'a> {
    src: &'a str,
    s: &'a [u8],
    lines: Vec<Line>,
    blk_indent: i32,
    line: usize,
    line_max: usize,
    tight: bool,
    list_indent: i32,
    parent: Parent,
    level: u32,
    /// `get_lines` scratch: one `(first, last, pad)` per line of the block being cut.
    pieces: Vec<(usize, usize, usize)>,
    out: Blocks<'a>,
}

/// Parses `src`, or returns `Err` if the line indexer met a CR or NUL that
/// markdown-it would normalize first; the caller normalizes and calls again.
pub fn parse(src: &str) -> Result<Blocks<'_>, ()> {
    let (mut st, dirty) = State::new(src);
    if dirty {
        return Err(());
    }
    if !src.is_empty() {
        let lm = st.line_max;
        st.tokenize(0, lm);
    }
    Ok(st.out)
}

impl<'a> State<'a> {
    fn new(src: &'a str) -> (Self, bool) {
        let s = src.as_bytes();
        // Capacity guess. Counting newlines first costs a scan of the whole
        // source, more than the few amortized grows a miss costs.
        let n = s.len() / 24 + 8;
        let mut st = State {
            src,
            s,
            lines: Vec::with_capacity(n),
            blk_indent: 0,
            line: 0,
            line_max: 0,
            tight: false,
            list_indent: -1,
            parent: Parent::Root,
            level: 0,
            pieces: Vec::new(),
            out: Blocks {
                src,
                tokens: Vec::with_capacity(n),
                buf: String::with_capacity(src.len() + 16),
                refs: FxMap::default(),
            },
        };
        let len = s.len();
        let mut start = 0;
        let mut dirty = false;
        while start < len {
            let (mut pos, mut offset) = (start, 0i32);
            while pos < len && is_space(s[pos]) {
                offset += if s[pos] == b'\t' { 4 - offset % 4 } else { 1 };
                pos += 1;
            }
            if pos == len {
                break;
            }
            let (nl, d) = find_newline(&s[pos..]);
            dirty |= d;
            let eol = match nl {
                Some(i) => pos + i,
                None => len,
            };
            st.lines.push(Line {
                start,
                end: eol,
                t_shift: pos - start,
                s_count: offset,
                bs_count: 0,
            });
            start = eol + 1;
        }
        st.lines.push(Line { start: len, end: len, t_shift: 0, s_count: 0, bs_count: 0 });
        st.line_max = st.lines.len() - 1;
        (st, dirty)
    }

    #[inline]
    fn ch(&self, pos: usize) -> u8 {
        if pos < self.s.len() {
            self.s[pos]
        } else {
            0
        }
    }

    fn push(&mut self, kind: Kind) -> usize {
        let n = kind.nesting();
        if n < 0 {
            self.level -= 1;
        }
        let level = self.level;
        if n > 0 {
            self.level += 1;
        }
        self.out.tokens.push(Token { kind, hidden: false, level, c0: 0, c1: 0, i0: 0, i1: 0 });
        self.out.tokens.len() - 1
    }

    fn push_content(&mut self, kind: Kind, c0: u32, c1: u32) -> usize {
        let i = self.push(kind);
        let t = &mut self.out.tokens[i];
        t.c0 = c0;
        t.c1 = c1;
        i
    }

    /// Copies `s` into the arena and returns its span.
    fn push_str(&mut self, s: &str) -> (u32, u32) {
        let a = self.out.buf.len();
        self.out.buf.push_str(s);
        (a as u32 | ARENA, self.out.buf.len() as u32)
    }

    fn is_empty(&self, line: usize) -> bool {
        self.lines[line].start + self.lines[line].t_shift >= self.lines[line].end
    }

    fn skip_empty_lines(&self, mut from: usize) -> usize {
        while from < self.line_max {
            if self.lines[from].start + self.lines[from].t_shift < self.lines[from].end {
                break;
            }
            from += 1;
        }
        from
    }

    fn skip_spaces(&self, mut pos: usize) -> usize {
        while pos < self.s.len() && is_space(self.s[pos]) {
            pos += 1;
        }
        pos
    }

    fn skip_spaces_back(&self, mut pos: usize, min: usize) -> usize {
        if pos <= min {
            return pos;
        }
        while pos > min {
            pos -= 1;
            if !is_space(self.s[pos]) {
                return pos + 1;
            }
        }
        pos
    }

    fn skip_chars(&self, mut pos: usize, c: u8) -> usize {
        while pos < self.s.len() && self.s[pos] == c {
            pos += 1;
        }
        pos
    }

    fn skip_chars_back(&self, mut pos: usize, c: u8, min: usize) -> usize {
        if pos <= min {
            return pos;
        }
        while pos > min {
            pos -= 1;
            if c != self.s[pos] {
                return pos + 1;
            }
        }
        pos
    }

    /// `getLines(begin, end, indent, keep_last_lf)`. When the stripped lines are adjacent
    /// in the source and need no padding (a column-0 paragraph, a fence body, a heading)
    /// the result is a span into `src`; otherwise the lines are copied into the arena.
    fn get_lines(&mut self, begin: usize, end: usize, indent: i32, keep_last_lf: bool) -> (u32, u32) {
        let slen = self.s.len();
        self.pieces.clear();
        let mut contiguous = true;
        for line in begin..end {
            let mut line_indent = 0i32;
            let line_start = self.lines[line].start;
            let mut first = line_start;
            let last = if line + 1 < end || keep_last_lf {
                (self.lines[line].end + 1).min(slen)
            } else {
                self.lines[line].end
            };
            while first < last && line_indent < indent {
                let ch = self.s[first];
                if is_space(ch) {
                    if ch == b'\t' {
                        line_indent += 4 - (line_indent + self.lines[line].bs_count) % 4;
                    } else {
                        line_indent += 1;
                    }
                } else if first - line_start < self.lines[line].t_shift {
                    line_indent += 1;
                } else {
                    break;
                }
                first += 1;
            }
            let pad = (line_indent - indent).max(0) as usize;
            if pad != 0 || self.pieces.last().is_some_and(|p| p.1 != first) {
                contiguous = false;
            }
            self.pieces.push((first, last, pad));
        }
        if contiguous {
            let a = self.pieces.first().map_or(0, |p| p.0);
            let b = self.pieces.last().map_or(0, |p| p.1);
            return (a as u32, b as u32);
        }
        let a = self.out.buf.len();
        for i in 0..self.pieces.len() {
            let (first, last, pad) = self.pieces[i];
            for _ in 0..pad {
                self.out.buf.push(' ');
            }
            self.out.buf.push_str(&self.src[first..last]);
        }
        (a as u32 | ARENA, self.out.buf.len() as u32)
    }

    fn get_lines_trimmed(&mut self, begin: usize, end: usize, indent: i32) -> (u32, u32) {
        let (a, b) = self.get_lines(begin, end, indent, false);
        let s = self.out.slice(a, b);
        let t = ascii_trim(s);
        let lead = (t.as_ptr() as usize - s.as_ptr() as usize) as u32;
        let trail = (s.len() - t.len()) as u32 - lead;
        (a + lead, b - trail)
    }

    /// The code rule appends '\n' to its lines. A source span can take the newline that
    /// already follows it; only a span at the very end of the input needs a copy.
    fn with_trailing_lf(&mut self, a: u32, b: u32) -> (u32, u32) {
        if a & ARENA != 0 {
            debug_assert_eq!(b as usize, self.out.buf.len());
            self.out.buf.push('\n');
        } else if self.s.get(b as usize) != Some(&b'\n') {
            let src = self.src;
            let (a, _) = self.push_str(&src[a as usize..b as usize]);
            self.out.buf.push('\n');
            return (a, self.out.buf.len() as u32);
        }
        (a, b + 1)
    }

    fn tokenize(&mut self, start_line: usize, end_line: usize) {
        let mut line = start_line;
        let mut has_empty_lines = false;
        while line < end_line {
            line = self.skip_empty_lines(line);
            self.line = line;
            if line >= end_line {
                break;
            }
            if self.lines[line].s_count < self.blk_indent {
                break;
            }
            if self.level >= MAX_NESTING {
                self.line = end_line;
                break;
            }
            let ok = self.table(line, end_line, false)
                || self.code(line, end_line)
                || match self.first_byte(line) {
                    b'`' | b'~' => self.fence(line, end_line, false),
                    b'>' => self.blockquote(line, end_line, false),
                    b'*' | b'-' => self.hr(line, end_line, false) || self.list(line, end_line, false),
                    b'_' => self.hr(line, end_line, false),
                    b'+' | b'0'..=b'9' => self.list(line, end_line, false),
                    b'[' => self.reference(line, end_line, false),
                    b'<' => self.html_block(line, end_line, false),
                    b'#' => self.heading(line, end_line, false),
                    _ => false,
                }
                || self.lheading_or_paragraph(line, end_line);
            debug_assert!(ok);
            self.tight = !has_empty_lines;
            if self.is_empty(self.line - 1) {
                has_empty_lines = true;
            }
            line = self.line;
            if line < end_line && self.is_empty(line) {
                has_empty_lines = true;
                line += 1;
                self.line = line;
            }
        }
    }

    /// First byte of `line` after its indent, 0 when the line is blank.
    #[inline]
    fn first_byte(&self, line: usize) -> u8 {
        self.ch(self.lines[line].start + self.lines[line].t_shift)
    }

    /// Every rule but `table` rejects on the first byte before touching state,
    /// so the chain dispatches on that byte and keeps the original rule order
    /// within each arm.
    fn terminates(&mut self, chain: Chain, line: usize, end: usize) -> bool {
        let c = self.first_byte(line);
        match chain {
            Chain::Paragraph => {
                self.table(line, end, true)
                    || match c {
                        b'`' | b'~' => self.fence(line, end, true),
                        b'>' => self.blockquote(line, end, true),
                        b'*' | b'-' => self.hr(line, end, true) || self.list(line, end, true),
                        b'_' => self.hr(line, end, true),
                        b'+' | b'0'..=b'9' => self.list(line, end, true),
                        b'<' => self.html_block(line, end, true),
                        b'#' => self.heading(line, end, true),
                        _ => false,
                    }
            }
            Chain::Blockquote => match c {
                b'`' | b'~' => self.fence(line, end, true),
                b'>' => self.blockquote(line, end, true),
                b'*' | b'-' => self.hr(line, end, true) || self.list(line, end, true),
                b'_' => self.hr(line, end, true),
                b'+' | b'0'..=b'9' => self.list(line, end, true),
                b'<' => self.html_block(line, end, true),
                b'#' => self.heading(line, end, true),
                _ => false,
            },
            Chain::List => match c {
                b'`' | b'~' => self.fence(line, end, true),
                b'>' => self.blockquote(line, end, true),
                b'*' | b'-' | b'_' => self.hr(line, end, true),
                _ => false,
            },
        }
    }

    fn line_text(&self, line: usize) -> &'a str {
        let pos = self.lines[line].start + self.lines[line].t_shift;
        let max = self.lines[line].end;
        if pos >= max {
            ""
        } else {
            &self.src[pos..max]
        }
    }

    // ---- rules ----

    fn code(&mut self, start_line: usize, end_line: usize) -> bool {
        if self.lines[start_line].s_count - self.blk_indent < 4 {
            return false;
        }
        let mut next = start_line + 1;
        let mut last = next;
        while next < end_line {
            if self.is_empty(next) {
                next += 1;
                continue;
            }
            if self.lines[next].s_count - self.blk_indent >= 4 {
                next += 1;
                last = next;
                continue;
            }
            break;
        }
        self.line = last;
        let (a, b) = self.get_lines(start_line, last, 4 + self.blk_indent, false);
        let (a, b) = self.with_trailing_lf(a, b);
        self.push_content(Kind::CodeBlock, a, b);
        true
    }

    fn fence(&mut self, start_line: usize, end_line: usize, silent: bool) -> bool {
        let mut pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        let mut max = self.lines[start_line].end;
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        if pos + 3 > max {
            return false;
        }
        let marker = self.s[pos];
        if marker != b'~' && marker != b'`' {
            return false;
        }
        let mut mem = pos;
        pos = self.skip_chars(pos, marker);
        let len = pos - mem;
        if len < 3 {
            return false;
        }
        let params = &self.src[pos..max];
        if marker == b'`' && params.as_bytes().contains(&b'`') {
            return false;
        }
        if silent {
            return true;
        }
        let mut next = start_line;
        let mut have_end = false;
        loop {
            next += 1;
            if next >= end_line {
                break;
            }
            pos = self.lines[next].start + self.lines[next].t_shift;
            mem = pos;
            max = self.lines[next].end;
            if pos < max && self.lines[next].s_count < self.blk_indent {
                break;
            }
            if self.ch(pos) != marker {
                continue;
            }
            if self.lines[next].s_count - self.blk_indent >= 4 {
                continue;
            }
            pos = self.skip_chars(pos, marker);
            if pos - mem < len {
                continue;
            }
            pos = self.skip_spaces(pos);
            if pos < max {
                continue;
            }
            have_end = true;
            break;
        }
        let indent = self.lines[start_line].s_count;
        self.line = next + have_end as usize;
        let (i0, i1) = self.push_str(params);
        let (c0, c1) = self.get_lines(start_line + 1, next, indent, true);
        let t = self.push_content(Kind::Fence, c0, c1);
        self.out.tokens[t].i0 = i0;
        self.out.tokens[t].i1 = i1;
        true
    }

    fn blockquote(&mut self, start_line: usize, end_line: usize, silent: bool) -> bool {
        let mut pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        let mut max;
        let old_line_max = self.line_max;
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        if self.ch(pos) != b'>' {
            return false;
        }
        if silent {
            return true;
        }
        let mut old_lines = Vec::new();
        let old_parent = self.parent;
        self.parent = Parent::Blockquote;
        let mut last_line_empty = false;
        let mut next = start_line;
        while next < end_line {
            let is_outdented = self.lines[next].s_count < self.blk_indent;
            pos = self.lines[next].start + self.lines[next].t_shift;
            max = self.lines[next].end;
            if pos >= max {
                break;
            }
            let c = self.s[pos];
            pos += 1;
            if c == b'>' && !is_outdented {
                let mut initial = self.lines[next].s_count + 1;
                let space_after;
                let mut adjust_tab = false;
                if self.ch(pos) == b' ' {
                    pos += 1;
                    initial += 1;
                    space_after = true;
                } else if self.ch(pos) == b'\t' {
                    space_after = true;
                    if (self.lines[next].bs_count + initial) % 4 == 3 {
                        pos += 1;
                        initial += 1;
                    } else {
                        adjust_tab = true;
                    }
                } else {
                    space_after = false;
                }
                let mut offset = initial;
                old_lines.push(self.lines[next]);
                self.lines[next].start = pos;
                while pos < max {
                    let ch = self.s[pos];
                    if is_space(ch) {
                        if ch == b'\t' {
                            offset += 4 - (offset + self.lines[next].bs_count + adjust_tab as i32) % 4;
                        } else {
                            offset += 1;
                        }
                    } else {
                        break;
                    }
                    pos += 1;
                }
                last_line_empty = pos >= max;
                self.lines[next].bs_count = self.lines[next].s_count + 1 + space_after as i32;
                self.lines[next].s_count = offset - initial;
                self.lines[next].t_shift = pos - self.lines[next].start;
                next += 1;
                continue;
            }
            if last_line_empty {
                break;
            }
            if self.terminates(Chain::Blockquote, next, end_line) {
                self.line_max = next;
                if self.blk_indent != 0 {
                    old_lines.push(self.lines[next]);
                    self.lines[next].s_count -= self.blk_indent;
                }
                break;
            }
            old_lines.push(self.lines[next]);
            self.lines[next].s_count = -1;
            next += 1;
        }
        let old_indent = self.blk_indent;
        self.blk_indent = 0;
        self.push(Kind::BlockquoteOpen);
        self.tokenize(start_line, next);
        self.push(Kind::BlockquoteClose);
        self.line_max = old_line_max;
        self.parent = old_parent;
        self.lines[start_line..start_line + old_lines.len()].copy_from_slice(&old_lines);
        self.blk_indent = old_indent;
        true
    }

    fn hr(&mut self, start_line: usize, _end: usize, silent: bool) -> bool {
        let max = self.lines[start_line].end;
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        let mut pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        let marker = self.ch(pos);
        pos += 1;
        if marker != b'*' && marker != b'-' && marker != b'_' {
            return false;
        }
        let mut cnt = 1;
        while pos < max {
            let ch = self.s[pos];
            pos += 1;
            if ch != marker && !is_space(ch) {
                return false;
            }
            if ch == marker {
                cnt += 1;
            }
        }
        if cnt < 3 {
            return false;
        }
        if silent {
            return true;
        }
        self.line = start_line + 1;
        self.push(Kind::Hr);
        true
    }

    fn skip_bullet_marker(&self, line: usize) -> Option<usize> {
        let max = self.lines[line].end;
        let mut pos = self.lines[line].start + self.lines[line].t_shift;
        let marker = self.ch(pos);
        pos += 1;
        if marker != b'*' && marker != b'-' && marker != b'+' {
            return None;
        }
        if pos < max && !is_space(self.s[pos]) {
            return None;
        }
        Some(pos)
    }

    fn skip_ordered_marker(&self, line: usize) -> Option<usize> {
        let start = self.lines[line].start + self.lines[line].t_shift;
        let max = self.lines[line].end;
        let mut pos = start;
        if pos + 1 >= max {
            return None;
        }
        let ch = self.s[pos];
        pos += 1;
        if !ch.is_ascii_digit() {
            return None;
        }
        loop {
            if pos >= max {
                return None;
            }
            let ch = self.s[pos];
            pos += 1;
            if ch.is_ascii_digit() {
                if pos - start >= 10 {
                    return None;
                }
                continue;
            }
            if ch == b')' || ch == b'.' {
                break;
            }
            return None;
        }
        if pos < max && !is_space(self.s[pos]) {
            return None;
        }
        Some(pos)
    }

    fn mark_tight_paragraphs(&mut self, idx: usize) {
        let level = self.level + 2;
        let toks = &mut self.out.tokens;
        let l = toks.len().saturating_sub(2);
        let mut i = idx + 2;
        while i < l {
            if toks[i].level == level && toks[i].kind == Kind::ParagraphOpen {
                toks[i + 2].hidden = true;
                toks[i].hidden = true;
                i += 2;
            }
            i += 1;
        }
    }

    fn list(&mut self, start_line: usize, end_line: usize, silent: bool) -> bool {
        let mut next = start_line;
        let mut tight = true;
        if self.lines[next].s_count - self.blk_indent >= 4 {
            return false;
        }
        if self.list_indent >= 0
            && self.lines[next].s_count - self.list_indent >= 4
            && self.lines[next].s_count < self.blk_indent
        {
            return false;
        }
        let mut is_terminating_paragraph = false;
        if silent && self.parent == Parent::Paragraph && self.lines[next].s_count >= self.blk_indent {
            is_terminating_paragraph = true;
        }
        let is_ordered;
        let mut marker_value = 0u32;
        let mut pos_after;
        if let Some(p) = self.skip_ordered_marker(next) {
            pos_after = p;
            is_ordered = true;
            let start = self.lines[next].start + self.lines[next].t_shift;
            marker_value = self.src[start..p - 1].parse().unwrap_or(0);
            if is_terminating_paragraph && marker_value != 1 {
                return false;
            }
        } else if let Some(p) = self.skip_bullet_marker(next) {
            pos_after = p;
            is_ordered = false;
        } else {
            return false;
        }
        if is_terminating_paragraph && self.skip_spaces(pos_after) >= self.lines[next].end {
            return false;
        }
        if silent {
            return true;
        }
        let marker_char = self.s[pos_after - 1];
        let list_tok_idx = self.out.tokens.len();
        if is_ordered {
            self.push(Kind::OrderedListOpen(marker_value));
        } else {
            self.push(Kind::BulletListOpen);
        }
        let mut prev_empty_end = false;
        let old_parent = self.parent;
        self.parent = Parent::List;
        while next < end_line {
            let mut pos = pos_after;
            let max = self.lines[next].end;
            let initial = self.lines[next].s_count + (pos_after - (self.lines[next].start + self.lines[next].t_shift)) as i32;
            let mut offset = initial;
            while pos < max {
                let ch = self.s[pos];
                if ch == b'\t' {
                    offset += 4 - (offset + self.lines[next].bs_count) % 4;
                } else if ch == b' ' {
                    offset += 1;
                } else {
                    break;
                }
                pos += 1;
            }
            let content_start = pos;
            let mut indent_after = if content_start >= max { 1 } else { offset - initial };
            if indent_after > 4 {
                indent_after = 1;
            }
            let indent = initial + indent_after;
            self.push(Kind::ListItemOpen);
            let old_tight = self.tight;
            let old_t_shift = self.lines[next].t_shift;
            let old_s_count = self.lines[next].s_count;
            let old_list_indent = self.list_indent;
            self.list_indent = self.blk_indent;
            self.blk_indent = indent;
            self.tight = true;
            self.lines[next].t_shift = content_start - self.lines[next].start;
            self.lines[next].s_count = offset;
            if content_start >= max && self.is_empty(next + 1) {
                self.line = (self.line + 2).min(end_line);
            } else {
                self.tokenize(next, end_line);
            }
            if !self.tight || prev_empty_end {
                tight = false;
            }
            prev_empty_end = (self.line - next) > 1 && self.is_empty(self.line - 1);
            self.blk_indent = self.list_indent;
            self.list_indent = old_list_indent;
            self.lines[next].t_shift = old_t_shift;
            self.lines[next].s_count = old_s_count;
            self.tight = old_tight;
            self.push(Kind::ListItemClose);
            next = self.line;
            if next >= end_line {
                break;
            }
            if self.lines[next].s_count < self.blk_indent {
                break;
            }
            if self.lines[next].s_count - self.blk_indent >= 4 {
                break;
            }
            if self.terminates(Chain::List, next, end_line) {
                break;
            }
            if is_ordered {
                match self.skip_ordered_marker(next) {
                    Some(p) => pos_after = p,
                    None => break,
                }
            } else {
                match self.skip_bullet_marker(next) {
                    Some(p) => pos_after = p,
                    None => break,
                }
            }
            if marker_char != self.s[pos_after - 1] {
                break;
            }
        }
        if is_ordered {
            self.push(Kind::OrderedListClose);
        } else {
            self.push(Kind::BulletListClose);
        }
        self.line = next;
        self.parent = old_parent;
        if tight {
            self.mark_tight_paragraphs(list_tok_idx);
        }
        true
    }

    fn line_end(&self, line: usize) -> usize {
        (self.lines[line].end + 1).min(self.s.len())
    }

    fn next_line_pos(&mut self, next: usize) -> Option<usize> {
        let end_line = self.line_max;
        if next >= end_line || self.is_empty(next) {
            return None;
        }
        let mut cont = false;
        if self.lines[next].s_count - self.blk_indent > 3 {
            cont = true;
        }
        if self.lines[next].s_count < 0 {
            cont = true;
        }
        if !cont {
            let old = self.parent;
            self.parent = Parent::Reference;
            let term = self.terminates(Chain::Paragraph, next, end_line);
            self.parent = old;
            if term {
                return None;
            }
        }
        Some(self.lines[next].start + self.lines[next].t_shift)
    }

    fn reference(&mut self, start_line: usize, _end: usize, silent: bool) -> bool {
        let s = self.s;
        let src = self.src;
        let mut pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        let mut next = start_line + 1;
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        if self.ch(pos) != b'[' {
            return false;
        }
        let mut max = self.line_end(start_line);
        let mut raw_label = String::new();
        let mut label_end = None;
        pos += 1;
        let mut seg = pos;
        while pos < max {
            if raw_label.len() + pos - seg > 999 * 2 * 3 {
                return false;
            }
            let mut ch = s[pos];
            if ch == b'[' {
                return false;
            }
            if ch == b']' {
                label_end = Some(pos);
                break;
            }
            if ch == b'\\' {
                pos += 1;
                ch = self.ch(pos);
            }
            if ch == b'\n' && pos < max {
                let Some(np) = self.next_line_pos(next) else { return false };
                raw_label.push_str(&src[seg..max]);
                pos = np;
                seg = np;
                max = self.line_end(next);
                next += 1;
                continue;
            }
            pos += 1;
        }
        let Some(label_end) = label_end else { return false };
        if self.ch(label_end + 1) != b':' {
            return false;
        }
        raw_label.push_str(&src[seg..label_end]);
        // MAX_LABEL_LENGTH counts UTF-16 units first, then code points
        if raw_label.len() > 999 {
            let u16len: usize = raw_label.chars().map(char::len_utf16).sum();
            if u16len > 999 && raw_label.chars().count() > 999 {
                return false;
            }
        }
        pos = label_end + 2;
        while pos < max {
            let ch = s[pos];
            if ch == b'\n' {
                let Some(np) = self.next_line_pos(next) else { break };
                pos = np;
                max = self.line_end(next);
                next += 1;
            } else if is_space(ch) {
                pos += 1;
            } else {
                break;
            }
        }
        let Some(dest) = parse_link_destination(src, pos, max) else { return false };
        let href = normalize_link(dest.str);
        if !validate_link(&href) {
            return false;
        }
        pos = dest.pos;
        let dest_end_pos = pos;
        let dest_end_max = max;
        let dest_end_line = next;
        while pos < max {
            let ch = s[pos];
            if ch == b'\n' {
                let Some(np) = self.next_line_pos(next) else { break };
                pos = np;
                max = self.line_end(next);
                next += 1;
            } else if is_space(ch) {
                pos += 1;
            } else {
                break;
            }
        }
        let title_start = pos;
        let mut tr = parse_link_title(src, pos, max, None);
        while tr.can_continue {
            let Some(np) = self.next_line_pos(next) else { break };
            pos = np;
            max = self.line_end(next);
            next += 1;
            tr = parse_link_title(src, pos, max, Some(tr));
        }
        let mut title;
        if title_start != dest_end_pos && tr.ok {
            title = tr.str;
            pos = tr.pos;
        } else {
            title = String::new();
            pos = dest_end_pos;
            max = dest_end_max;
            next = dest_end_line;
        }
        while pos < max && is_space(s[pos]) {
            pos += 1;
        }
        if pos < max && s[pos] != b'\n' && !title.is_empty() {
            title = String::new();
            pos = dest_end_pos;
            max = dest_end_max;
            next = dest_end_line;
            while pos < max && is_space(s[pos]) {
                pos += 1;
            }
        }
        if pos < max && s[pos] != b'\n' {
            return false;
        }
        let label = normalize_reference(&raw_label);
        if label.is_empty() {
            return false;
        }
        if silent {
            return true;
        }
        self.out.refs.entry(label).or_insert(Reference { href: href.into_owned(), title });
        self.line = next;
        true
    }

    fn html_block(&mut self, start_line: usize, end_line: usize, silent: bool) -> bool {
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        let pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        if self.ch(pos) != b'<' {
            return false;
        }
        let line_text = &self.src[pos..self.lines[start_line].end];
        let Some(seq) = html::block_sequence(line_text) else { return false };
        if silent {
            return seq != 6;
        }
        let mut next = start_line + 1;
        let ends_on_blank = seq >= 5;
        if !html::block_sequence_closes(seq, line_text) {
            while next < end_line {
                if self.lines[next].s_count < self.blk_indent && (ends_on_blank || !self.is_empty(next)) {
                    break;
                }
                let lt = self.line_text(next);
                if html::block_sequence_closes(seq, lt) {
                    if !lt.is_empty() {
                        next += 1;
                    }
                    break;
                }
                next += 1;
            }
        }
        self.line = next;
        let bi = self.blk_indent;
        let (a, b) = self.get_lines(start_line, next, bi, true);
        self.push_content(Kind::HtmlBlock, a, b);
        true
    }

    fn heading(&mut self, start_line: usize, _end: usize, silent: bool) -> bool {
        let mut pos = self.lines[start_line].start + self.lines[start_line].t_shift;
        let mut max = self.lines[start_line].end;
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        if self.ch(pos) != b'#' || pos >= max {
            return false;
        }
        let mut level = 1u8;
        pos += 1;
        while pos < max && self.s[pos] == b'#' && level <= 6 {
            level += 1;
            pos += 1;
        }
        if level > 6 || (pos < max && !is_space(self.s[pos])) {
            return false;
        }
        if silent {
            return true;
        }
        max = self.skip_spaces_back(max, pos);
        let tmp = self.skip_chars_back(max, b'#', pos);
        if tmp > pos && is_space(self.s[tmp - 1]) {
            max = tmp;
        }
        self.line = start_line + 1;
        self.push(Kind::HeadingOpen(level));
        let content = if pos < max { ascii_trim(&self.src[pos..max]) } else { "" };
        let (a, b) = self.push_str(content);
        self.push_content(Kind::Inline, a, b);
        self.push(Kind::HeadingClose(level));
        true
    }

    /// lheading and paragraph walk the same continuation lines with the same
    /// skip and terminator checks; only lheading's setext marker test differs,
    /// and when no marker is found its `next` is the paragraph's `next`. One
    /// walk serves both, so each line is classified once.
    fn lheading_or_paragraph(&mut self, start_line: usize, end_line: usize) -> bool {
        let may_head = self.lines[start_line].s_count - self.blk_indent < 4;
        let old_parent = self.parent;
        self.parent = Parent::Paragraph;
        let mut level = 0u8;
        let mut next = start_line + 1;
        while next < end_line && !self.is_empty(next) {
            if self.lines[next].s_count - self.blk_indent > 3 {
                next += 1;
                continue;
            }
            if may_head && self.lines[next].s_count >= self.blk_indent {
                let mut pos = self.lines[next].start + self.lines[next].t_shift;
                let max = self.lines[next].end;
                if pos < max {
                    let marker = self.s[pos];
                    if marker == b'-' || marker == b'=' {
                        pos = self.skip_chars(pos, marker);
                        pos = self.skip_spaces(pos);
                        if pos >= max {
                            level = if marker == b'=' { 1 } else { 2 };
                            break;
                        }
                    }
                }
            }
            if self.lines[next].s_count < 0 {
                next += 1;
                continue;
            }
            if self.terminates(Chain::Paragraph, next, end_line) {
                break;
            }
            next += 1;
        }
        let bi = self.blk_indent;
        let (a, b) = self.get_lines_trimmed(start_line, next, bi);
        if level == 0 {
            self.line = next;
            self.push(Kind::ParagraphOpen);
            self.push_content(Kind::Inline, a, b);
            self.push(Kind::ParagraphClose);
        } else {
            self.line = next + 1;
            self.push(Kind::HeadingOpen(level));
            self.push_content(Kind::Inline, a, b);
            self.push(Kind::HeadingClose(level));
        }
        self.parent = old_parent;
        true
    }

    fn table(&mut self, start_line: usize, end_line: usize, silent: bool) -> bool {
        if start_line + 2 > end_line {
            return false;
        }
        let mut next = start_line + 1;
        if self.lines[next].s_count < self.blk_indent {
            return false;
        }
        if self.lines[next].s_count - self.blk_indent >= 4 {
            return false;
        }
        let mut pos = self.lines[next].start + self.lines[next].t_shift;
        let em = self.lines[next].end;
        if pos >= em {
            return false;
        }
        let first = self.s[pos];
        pos += 1;
        if first != b'|' && first != b'-' && first != b':' {
            return false;
        }
        if pos >= em {
            return false;
        }
        let second = self.s[pos];
        pos += 1;
        if second != b'|' && second != b'-' && second != b':' && !is_space(second) {
            return false;
        }
        if first == b'-' && is_space(second) {
            return false;
        }
        while pos < em {
            let ch = self.s[pos];
            if ch != b'|' && ch != b'-' && ch != b':' && !is_space(ch) {
                return false;
            }
            pos += 1;
        }
        let line_text = self.line_text(start_line + 1);
        let cols: Vec<&str> = line_text.split('|').collect();
        let mut aligns: Vec<u8> = Vec::new();
        let nc = cols.len();
        for (i, c) in cols.iter().enumerate() {
            let t = js_trim(c).as_bytes();
            if t.is_empty() {
                if i == 0 || i == nc - 1 {
                    continue;
                }
                return false;
            }
            let body = t.strip_prefix(b":").unwrap_or(t);
            let body = body.strip_suffix(b":").unwrap_or(body);
            if body.is_empty() || !body.iter().all(|&c| c == b'-') {
                return false;
            }
            let last = t[t.len() - 1] == b':';
            let firstc = t[0] == b':';
            aligns.push(match (firstc, last) {
                (true, true) => 1,
                (false, true) => 2,
                (true, false) => 3,
                _ => 0,
            });
        }
        let header = js_trim(self.line_text(start_line));
        if !header.contains('|') {
            return false;
        }
        if self.lines[start_line].s_count - self.blk_indent >= 4 {
            return false;
        }
        let mut columns = escaped_split(header);
        trim_edge_columns(&mut columns);
        let column_count = columns.len();
        if column_count == 0 || column_count != aligns.len() {
            return false;
        }
        if silent {
            return true;
        }
        let old_parent = self.parent;
        self.parent = Parent::Table;
        self.push(Kind::TableOpen);
        self.push(Kind::TheadOpen);
        self.push(Kind::TrOpen);
        for (i, c) in columns.iter().enumerate() {
            self.push(Kind::ThOpen(aligns[i]));
            let (a, b) = self.push_str(js_trim(c));
            self.push_content(Kind::Inline, a, b);
            self.push(Kind::ThClose);
        }
        self.push(Kind::TrClose);
        self.push(Kind::TheadClose);
        let mut has_body = false;
        let mut autocompleted: i64 = 0;
        next = start_line + 2;
        while next < end_line {
            if self.lines[next].s_count < self.blk_indent {
                break;
            }
            if self.terminates(Chain::Blockquote, next, end_line) {
                break;
            }
            let lt = js_trim(self.line_text(next));
            if lt.is_empty() {
                break;
            }
            if self.lines[next].s_count - self.blk_indent >= 4 {
                break;
            }
            let mut columns = escaped_split(lt);
            trim_edge_columns(&mut columns);
            autocompleted += column_count as i64 - columns.len() as i64;
            if autocompleted > 0x10000 {
                break;
            }
            if next == start_line + 2 {
                self.push(Kind::TbodyOpen);
                has_body = true;
            }
            self.push(Kind::TrOpen);
            for i in 0..column_count {
                self.push(Kind::TdOpen(aligns[i]));
                let cell = columns.get(i).map(|c| js_trim(c)).unwrap_or("");
                let (a, b) = self.push_str(cell);
                self.push_content(Kind::Inline, a, b);
                self.push(Kind::TdClose);
            }
            self.push(Kind::TrClose);
            next += 1;
        }
        if has_body {
            self.push(Kind::TbodyClose);
        }
        self.push(Kind::TableClose);
        self.parent = old_parent;
        self.line = next;
        true
    }
}

fn trim_edge_columns(columns: &mut Vec<alloc::borrow::Cow<'_, str>>) {
    if !columns.is_empty() && columns[0].is_empty() {
        columns.remove(0);
    }
    if !columns.is_empty() && columns[columns.len() - 1].is_empty() {
        columns.pop();
    }
}

fn escaped_split(s: &str) -> Vec<alloc::borrow::Cow<'_, str>> {
    use alloc::borrow::Cow;
    let b = s.as_bytes();
    let mut result: Vec<Cow<str>> = Vec::new();
    let mut is_escaped = false;
    let mut last = 0;
    let mut current = String::new();
    let mut has_current = false;
    for (pos, &ch) in b.iter().enumerate() {
        if ch == b'|' {
            if !is_escaped {
                if has_current {
                    current.push_str(&s[last..pos]);
                    result.push(Cow::Owned(core::mem::take(&mut current)));
                    has_current = false;
                } else {
                    result.push(Cow::Borrowed(&s[last..pos]));
                }
                last = pos + 1;
            } else {
                current.push_str(&s[last..pos - 1]);
                has_current = true;
                last = pos;
            }
        }
        is_escaped = ch == b'\\';
    }
    if has_current {
        current.push_str(&s[last..]);
        result.push(Cow::Owned(current));
    } else {
        result.push(Cow::Borrowed(&s[last..]));
    }
    result
}

/// Position of the first newline in `s`, and whether a CR or NUL was seen
/// on the way (through the end of the chunk holding that newline). The flag
/// only decides whether the source gets normalized and reparsed, so seeing
/// a CR past the newline is harmless.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn find_newline(s: &[u8]) -> (Option<usize>, bool) {
    use core::arch::wasm32::*;
    let n = s.len();
    if n < 16 {
        return find_newline_swar(s);
    }
    let p = s.as_ptr();
    let (nl, cr, nul) = (u8x16_splat(b'\n'), u8x16_splat(b'\r'), u8x16_splat(0));
    let mut dirty = u8x16_splat(0);
    let mut i = 0;
    while i + 16 <= n {
        // Callers keep i + 16 <= n, so the load stays inside s.
        let v = unsafe { v128_load(p.add(i) as *const v128) };
        dirty = v128_or(dirty, v128_or(u8x16_eq(v, cr), u8x16_eq(v, nul)));
        let m = u8x16_bitmask(u8x16_eq(v, nl));
        if m != 0 {
            return (Some(i + m.trailing_zeros() as usize), v128_any_true(dirty));
        }
        i += 16;
    }
    if i < n {
        // Overlapping tail: reload the last 16 bytes and drop the bits already scanned.
        let off = n - 16;
        let v = unsafe { v128_load(p.add(off) as *const v128) };
        dirty = v128_or(dirty, v128_or(u8x16_eq(v, cr), u8x16_eq(v, nul)));
        let m = (u8x16_bitmask(u8x16_eq(v, nl)) as u32) >> (i - off);
        if m != 0 {
            return (Some(i + m.trailing_zeros() as usize), v128_any_true(dirty));
        }
    }
    (None, v128_any_true(dirty))
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
#[inline]
fn find_newline(s: &[u8]) -> (Option<usize>, bool) {
    find_newline_swar(s)
}

fn find_newline_swar(s: &[u8]) -> (Option<usize>, bool) {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    const NL: u64 = LO * b'\n' as u64;
    const CR: u64 = LO * b'\r' as u64;
    let mut dirty = 0u64;
    let mut i = 0;
    while i + 8 <= s.len() {
        let w = u64::from_le_bytes(s[i..i + 8].try_into().unwrap());
        let zero = |x: u64| x.wrapping_sub(LO) & !x & HI;
        dirty |= zero(w ^ CR) | zero(w);
        let z = zero(w ^ NL);
        if z != 0 {
            return (Some(i + (z.trailing_zeros() / 8) as usize), dirty != 0);
        }
        i += 8;
    }
    let mut d = dirty != 0;
    for (k, &b) in s[i..].iter().enumerate() {
        if b == b'\n' {
            return (Some(i + k), d);
        }
        d |= b == b'\r' || b == 0;
    }
    (None, d)
}


