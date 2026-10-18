//! Native conformance against the markdown-it oracle in `bench/`.
//!
//! `fixtures_match_oracle` needs `node` and `bench/node_modules`; it pipes every fixture and
//! corpus case through markdown-it and asserts byte-identical HTML. `regressions` is
//! self-contained and pins outputs that once differed from the oracle.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Emits each case as `id\n`, `len\n` + src bytes, `len\n` + expected bytes.
const DUMP_JS: &str = r#"
const bench = process.argv[2];
const { fixtureSuites } = await import(`file://${bench}/fixtures.mjs`);
const { corpora, loadCorpus } = await import(`file://${bench}/corpora.mjs`);
const { load } = await import(`file://${bench}/impls.mjs`);
const oracle = await load('markdown-it');
const suites = [
  ...Object.entries(fixtureSuites).map(([name, cases]) => [name, cases]),
  ...Object.keys(corpora).map((name) => [name, () => loadCorpus(name).map((c) => ({ id: c.path, src: c.src }))]),
];
const frames = [];
const frame = (s) => { const b = Buffer.from(s, 'utf8'); frames.push(Buffer.from(b.length + '\n'), b); };
for (const [suite, cases] of suites) {
  for (const c of cases()) {
    frames.push(Buffer.from(`${suite} ${c.id}\n`));
    frame(c.src);
    frame(oracle(c.src));
  }
}
process.stdout.write(Buffer.concat(frames));
"#;

struct Case {
    id: String,
    src: String,
    expected: String,
}

fn oracle_cases() -> Vec<Case> {
    let bench = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench").canonicalize().unwrap();
    let mut child = Command::new("node")
        .args(["--input-type=module", "-", bench.to_str().unwrap()])
        .current_dir(&bench)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("node is required for the oracle test");
    child.stdin.take().unwrap().write_all(DUMP_JS.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "oracle dump failed");
    let mut r = Reader { b: &out.stdout, pos: 0 };
    let mut cases = Vec::new();
    while r.pos < r.b.len() {
        let id = r.line();
        let src = r.blob();
        let expected = r.blob();
        cases.push(Case { id, src, expected });
    }
    cases
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn line(&mut self) -> String {
        let nl = self.pos + self.b[self.pos..].iter().position(|&c| c == b'\n').unwrap();
        let s = std::str::from_utf8(&self.b[self.pos..nl]).unwrap().to_string();
        self.pos = nl + 1;
        s
    }

    fn blob(&mut self) -> String {
        let len: usize = self.line().parse().unwrap();
        let s = String::from_utf8(self.b[self.pos..self.pos + len].to_vec()).unwrap();
        self.pos += len;
        s
    }
}

fn first_diff(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

fn window(s: &str, at: usize) -> &str {
    let mut from = at.saturating_sub(120);
    let mut to = (at + 120).min(s.len());
    while !s.is_char_boundary(from) {
        from -= 1;
    }
    while !s.is_char_boundary(to) {
        to += 1;
    }
    &s[from..to]
}

#[test]
fn fixtures_match_oracle() {
    let cases = oracle_cases();
    assert!(cases.len() > 900, "oracle dump returned only {} cases", cases.len());
    let mut failures = Vec::new();
    for c in &cases {
        let actual = mdust::render(&c.src);
        if actual != c.expected {
            let at = first_diff(&c.expected, &actual);
            failures.push(format!(
                "{} (first diff at byte {at})\n  expected {:?}\n  actual   {:?}",
                c.id,
                window(&c.expected, at),
                window(&actual, at)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from markdown-it:\n{}",
        failures.len(),
        cases.len(),
        failures.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn regressions() {
    // markdown-it steps one UTF-16 unit past a failed inline destination before it looks for
    // a reference label, so a BMP char there is skipped whole and an astral char is not.
    let cases: &[(&str, &str)] = &[
        ("[](<>ﬀ[foo]\n\n[foo]:r\n", "<p><a href=\"r\"></a></p>\n"),
        ("[](<>é[foo]\n\n[foo]:r\n", "<p><a href=\"r\"></a></p>\n"),
        ("[a](<>ﬀ[foo]\n\n[foo]:r\n", "<p><a href=\"r\">a</a></p>\n"),
        ("[](<>😀[foo]\n\n[foo]:r\n", "<p>[](&lt;&gt;😀<a href=\"r\">foo</a></p>\n"),
        ("[](<>[foo]\n\n[foo]:r\n", "<p>[](&lt;&gt;<a href=\"r\">foo</a></p>\n"),
        ("![](<>ﬀ[foo]\n\n[foo]:r\n", "<p>!<a href=\"r\"></a></p>\n"),
    ];
    for (src, expected) in cases {
        assert_eq!(&mdust::render(src), expected, "input {src:?}");
    }
}

/// CR and NUL are normalized before parsing; the line indexer reports them
/// instead of a separate scan, so each position they can sit in is covered:
/// mid-line, after the last newline, inside leading whitespace, alone.
#[test]
fn cr_and_nul_normalize_like_markdown_it() {
    let cases: &[(&str, &str)] = &[
        ("a\r\nb\0c\n", "<p>a\nb\u{FFFD}c</p>\n"),
        ("# h\r\n\r\n> q\rz", "<h1>h</h1>\n<blockquote>\n<p>q\nz</p>\n</blockquote>\n"),
        ("\0", "<p>\u{FFFD}</p>\n"),
        ("x\r", "<p>x</p>\n"),
        ("  \r\n\ttab\0\r\n", "<pre><code>tab\u{FFFD}\n</code></pre>\n"),
        ("\r\n\r\n- a\r\n- b", "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"),
    ];
    for (src, expected) in cases {
        assert_eq!(&mdust::render(src), expected, "input {src:?}");
    }
}
