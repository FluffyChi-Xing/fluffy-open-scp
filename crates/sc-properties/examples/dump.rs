use sc_properties::PropertyFile;

fn main() {
    for path in std::env::args().skip(1) {
        println!("=== {path} ===");
        let data = std::fs::read(&path).unwrap();
        match PropertyFile::parse(&data) {
            Ok(file) => {
                println!("claimed={} parsed={}", file.claimed_count, file.values.len());
                print!("{}", file);
            }
            Err(e) => println!("parse error: {e}"),
        }
    }
}
