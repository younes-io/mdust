//! Native timing host for the mdust crate, same contract as `mdit-rs-native`:
//! `mdust-native <min_ms> <file>...` prints `{"msPerPass", "iters", "sink"}`.
//! `DUMP=<dir>` instead writes each input's render to `<dir>/<input path>.html`
//! for `bytecmp-native.mjs`. `MDUST_SIMD=sse2|ssse3|avx2` pins the scanner level.
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let min_ms: f64 = args[0].parse().unwrap();
    let docs: Vec<String> = args[1..].iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    if let Ok(l) = std::env::var("MDUST_SIMD") {
        mdust::force_simd_level(match l.as_str() { "sse2" => 1, "ssse3" => 2, "avx2" => 3, _ => panic!("MDUST_SIMD={l}") });
    }
    if let Some(dir) = std::env::var_os("DUMP") {
        for (p, d) in args[1..].iter().zip(&docs) {
            let out = Path::new(&dir).join(p.trim_start_matches('/')).with_extension("md.html");
            std::fs::create_dir_all(out.parent().unwrap()).unwrap();
            std::fs::write(out, mdust::render(d)).unwrap();
        }
        return;
    }
    let mut sink = 0usize;
    let mut pass = || for d in &docs { sink += mdust::render(d).len(); };
    let warm = Instant::now();
    while warm.elapsed().as_secs_f64() < 1.5 { pass(); }
    let t = Instant::now();
    let mut iters = 0u32;
    while t.elapsed().as_secs_f64() * 1000.0 < min_ms { pass(); iters += 1; }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("{{\"msPerPass\":{ms},\"iters\":{iters},\"sink\":{sink},\"simd\":{}}}", mdust::simd_level());
}
