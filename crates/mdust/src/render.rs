//! HTML output, a port of markdown-it's Renderer default rules.
use crate::prelude::*;

use crate::block::{Blocks, Kind};
use crate::inline::{IKind, Inline, Scratch};
use crate::utils::{escape_html_into, push_uint, is_js_space, js_trim, unescape_all};

pub fn render(blocks: &Blocks, out: &mut String) {
    let toks = &blocks.tokens;
    let mut scratch = Scratch::default();
    for (idx, t) in toks.iter().enumerate() {
        match t.kind {
            Kind::Inline => render_inline(blocks.content(t), blocks, &mut scratch, out),
            Kind::CodeBlock => {
                out.push_str("<pre><code>");
                escape_html_into(out, blocks.content(t));
                out.push_str("</code></pre>\n");
            }
            Kind::Fence => {
                let info = unescape_all(blocks.info(t));
                let info = js_trim(&info);
                if info.is_empty() {
                    out.push_str("<pre><code>");
                } else {
                    let lang = info.split(is_js_space).next().unwrap_or("");
                    out.push_str("<pre><code class=\"language-");
                    escape_html_into(out, lang);
                    out.push_str("\">");
                }
                escape_html_into(out, blocks.content(t));
                out.push_str("</code></pre>\n");
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
                out.push_str(if nesting == -1 { "</" } else { "<" });
                out.push_str(t.kind.tag());
                match t.kind {
                    Kind::OrderedListOpen(n) if n != 1 => {
                        out.push_str(" start=\"");
                        push_uint(out, n as u64);
                        out.push('"');
                    }
                    Kind::ThOpen(a) | Kind::TdOpen(a) if a != 0 => {
                        out.push_str(match a {
                            1 => " style=\"text-align:center\"",
                            2 => " style=\"text-align:right\"",
                            _ => " style=\"text-align:left\"",
                        });
                    }
                    _ => {}
                }
                let mut need_lf = true;
                if nesting == 1 {
                    if let Some(n) = toks.get(idx + 1) {
                        if n.kind == Kind::Inline
                            || n.hidden
                            || (n.kind.nesting() == -1 && n.kind.tag() == t.kind.tag())
                        {
                            need_lf = false;
                        }
                    }
                }
                out.push_str(if need_lf { ">\n" } else { ">" });
            }
        }
    }
}

fn render_inline(src: &str, blocks: &Blocks, scratch: &mut Scratch, out: &mut String) {
    let st = Inline::parse_with(src, &blocks.refs, scratch);
    for t in &st.tokens {
        match t.kind {
            IKind::Text | IKind::TextSpecial => escape_html_into(out, st.content(t)),
            IKind::Softbreak => out.push('\n'),
            IKind::Hardbreak => out.push_str("<br>\n"),
            IKind::CodeInline => {
                out.push_str("<code>");
                escape_html_into(out, st.content(t));
                out.push_str("</code>");
            }
            IKind::HtmlInline => out.push_str(st.content(t)),
            IKind::EmOpen => out.push_str("<em>"),
            IKind::EmClose => out.push_str("</em>"),
            IKind::StrongOpen => out.push_str("<strong>"),
            IKind::StrongClose => out.push_str("</strong>"),
            IKind::SOpen => out.push_str("<s>"),
            IKind::SClose => out.push_str("</s>"),
            IKind::LinkOpen => {
                out.push_str("<a href=\"");
                escape_html_into(out, st.content(t));
                out.push('"');
                let title = st.arena_str(t.t);
                if !title.is_empty() {
                    out.push_str(" title=\"");
                    escape_html_into(out, title);
                    out.push('"');
                }
                out.push('>');
            }
            IKind::LinkClose => out.push_str("</a>"),
            IKind::Image => {
                out.push_str("<img src=\"");
                escape_html_into(out, st.content(t));
                out.push_str("\" alt=\"");
                escape_html_into(out, st.arena_str(t.alt1));
                out.push('"');
                let title = st.arena_str(t.t);
                if !title.is_empty() {
                    out.push_str(" title=\"");
                    escape_html_into(out, title);
                    out.push('"');
                }
                out.push('>');
            }
        }
    }
    st.recycle(scratch);
}
