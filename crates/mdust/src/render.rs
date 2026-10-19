//! HTML output, a port of markdown-it's Renderer default rules.
use crate::prelude::*;

use crate::block::{Blocks, Kind};
use crate::inline::{IKind, Inline, Scratch};
use crate::utils::{escape_html_into, push_reserved, push_uint, is_js_space, js_trim, unescape_all};

/// Tags are short, so the copy goes through wide loads rather than memory.copy.
#[inline(always)]
fn tag(out: &mut String, s: &str) {
    out.reserve(s.len());
    push_reserved(out, s.as_bytes());
}

pub fn render(blocks: &Blocks, out: &mut String) {
    let toks = &blocks.tokens;
    let mut scratch = Scratch::default();
    for (idx, t) in toks.iter().enumerate() {
        match t.kind {
            Kind::Inline => scratch = render_inline(blocks.content(t), blocks, scratch, out),
            Kind::CodeBlock => {
                tag(out, "<pre><code>");
                escape_html_into(out, blocks.content(t));
                tag(out, "</code></pre>\n");
            }
            Kind::Fence => {
                let info = unescape_all(blocks.info(t));
                let info = js_trim(&info);
                if info.is_empty() {
                    tag(out, "<pre><code>");
                } else {
                    let lang = info.split(is_js_space).next().unwrap_or("");
                    tag(out, "<pre><code class=\"language-");
                    escape_html_into(out, lang);
                    tag(out, "\">");
                }
                escape_html_into(out, blocks.content(t));
                tag(out, "</code></pre>\n");
            }
            Kind::HtmlBlock => out.push_str(blocks.content(t)),
            _ => {
                if t.hidden {
                    continue;
                }
                let nesting = t.kind.nesting();
                if nesting != -1 && idx > 0 && toks[idx - 1].hidden && toks[idx - 1].kind.nesting() == -1 {
                    out.push('\n');
                }
                let lf = nesting != 1
                    || match toks.get(idx + 1) {
                        Some(n) => {
                            !(n.kind == Kind::Inline
                                || n.hidden
                                || (n.kind.nesting() == -1 && n.kind.tag() == t.kind.tag()))
                        }
                        None => true,
                    };
                match t.kind {
                    Kind::OrderedListOpen(n) if n != 1 => {
                        tag(out, "<ol start=\"");
                        push_uint(out, n as u64);
                        tag(out, if lf { "\">\n" } else { "\">" });
                    }
                    Kind::ThOpen(a) | Kind::TdOpen(a) if a != 0 => {
                        tag(out, match (t.kind, a) {
                            (Kind::ThOpen(_), 1) => "<th style=\"text-align:center\"",
                            (Kind::ThOpen(_), 2) => "<th style=\"text-align:right\"",
                            (Kind::ThOpen(_), _) => "<th style=\"text-align:left\"",
                            (_, 1) => "<td style=\"text-align:center\"",
                            (_, 2) => "<td style=\"text-align:right\"",
                            _ => "<td style=\"text-align:left\"",
                        });
                        tag(out, if lf { ">\n" } else { ">" });
                    }
                    k => tag(out, k.html(lf)),
                }
            }
        }
    }
}

fn render_inline(src: &str, blocks: &Blocks, scratch: Scratch, out: &mut String) -> Scratch {
    let st = Inline::parse_with(src, &blocks.refs, scratch);
    for t in &st.tokens {
        match t.kind {
            IKind::Text | IKind::TextSpecial => escape_html_into(out, st.content(t)),
            IKind::Softbreak => out.push('\n'),
            IKind::Hardbreak => tag(out, "<br>\n"),
            IKind::CodeInline => {
                tag(out, "<code>");
                escape_html_into(out, st.content(t));
                tag(out, "</code>");
            }
            IKind::HtmlInline => out.push_str(st.content(t)),
            IKind::EmOpen => tag(out, "<em>"),
            IKind::EmClose => tag(out, "</em>"),
            IKind::StrongOpen => tag(out, "<strong>"),
            IKind::StrongClose => tag(out, "</strong>"),
            IKind::SOpen => tag(out, "<s>"),
            IKind::SClose => tag(out, "</s>"),
            IKind::LinkOpen => {
                tag(out, "<a href=\"");
                escape_html_into(out, st.content(t));
                out.push('"');
                let title = st.arena_str(t.t);
                if !title.is_empty() {
                    tag(out, " title=\"");
                    escape_html_into(out, title);
                    out.push('"');
                }
                out.push('>');
            }
            IKind::LinkClose => tag(out, "</a>"),
            IKind::Image => {
                tag(out, "<img src=\"");
                escape_html_into(out, st.content(t));
                tag(out, "\" alt=\"");
                escape_html_into(out, st.arena_str(t.alt1));
                out.push('"');
                let title = st.arena_str(t.t);
                if !title.is_empty() {
                    tag(out, " title=\"");
                    escape_html_into(out, title);
                    out.push('"');
                }
                out.push('>');
            }
        }
    }
    st.recycle()
}
