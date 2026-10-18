use std::time::Instant;

fn main() {
    let files: Vec<String> = std::env::args().skip(1).map(|p| std::fs::read_to_string(p).unwrap()).collect();
    let bytes: usize = files.iter().map(|s| s.len()).sum();
    let mut sink = 0;
    for _ in 0..20 {
        for f in &files {
            sink += mdust::render(f).len();
        }
    }
    let t = Instant::now();
    let mut n = 0;
    while t.elapsed().as_millis() < 2000 {
        for f in &files {
            sink += mdust::render(f).len();
        }
        n += 1;
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("{} files {} KiB: {:.3} ms/pass, {:.1} MiB/s (sink {})", files.len(), bytes / 1024, ms, bytes as f64 / 1048576.0 / (ms / 1000.0), sink);
}
