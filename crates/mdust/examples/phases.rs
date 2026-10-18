use std::time::Instant;
fn time(name: &str, files: &[String], f: impl Fn(&str) -> usize) {
    let mut sink = 0;
    for _ in 0..20 { for s in files { sink += f(s); } }
    let t = Instant::now();
    let mut n = 0;
    while t.elapsed().as_millis() < 1500 { for s in files { sink += f(s); } n += 1; }
    println!("{name}: {:.3} ms/pass ({sink})", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
}
fn main() {
    let files: Vec<String> = std::env::args().skip(1).map(|p| std::fs::read_to_string(p).unwrap()).collect();
    time("blocks", &files, mdust::profile_blocks);
    time("blocks+inline", &files, mdust::profile_inline_parse);
    time("full", &files, |s| mdust::render(s).len());
}
