use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let min_ms: f64 = args[0].parse().unwrap();
    let docs: Vec<String> = args[1..].iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    let mut sink = 0usize;
    let mut pass = || for d in &docs { sink += if std::env::var_os("PARSE_ONLY").is_some() { mdit_rs_wasm::parse_only(d) } else { mdit_rs_wasm::render(d).len() }; };
    let warm = Instant::now();
    while warm.elapsed().as_secs_f64() < 1.5 { pass(); }
    let t = Instant::now();
    let mut iters = 0u32;
    while t.elapsed().as_secs_f64() * 1000.0 < min_ms { pass(); iters += 1; }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("{{\"msPerPass\":{ms},\"iters\":{iters},\"sink\":{sink}}}");
}
