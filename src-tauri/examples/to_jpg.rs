fn main() {
    let args: Vec<String> = std::env::args().collect();
    let img = image::open(&args[1]).expect("open image");
    let out = std::fs::File::create(&args[2]).expect("create out");
    let writer = std::io::BufWriter::new(out);
    let rgb = image::DynamicImage::ImageRgb8(img.to_rgb8());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 85);
    encoder.encode_image(&rgb).expect("encode");
}
