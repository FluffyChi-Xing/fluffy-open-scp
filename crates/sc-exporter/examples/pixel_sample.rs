//! 一次性采样工具：打印指定 PNG 指定点位的 RGBA（诊断用，可随时删除）。
//! 用法：pixel_sample <png> <x>,<y>:<label> ...

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        panic!("usage: pixel_sample <png> <x>,<y>[:label] ...");
    }
    let image = image::open(&args[0]).expect("open png").to_rgba8();
    println!(
        "== {}  {}x{}",
        args[0],
        image.width(),
        image.height()
    );
    for point in &args[1..] {
        let (xy, label) = point.split_once(':').unwrap_or((point.as_str(), ""));
        let (x, y) = xy.split_once(',').expect("x,y");
        let (x, y): (u32, u32) = (x.parse().unwrap(), y.parse().unwrap());
        let px = image.get_pixel(x, y);
        println!("  ({x},{y}) {label} = RGB({},{},{})", px[0], px[1], px[2]);
    }
}
