//! 打开一个 .package 并打印概况：
//!
//! ```text
//! cargo run -p dbpf --example inspect -- path/to/file.package [dump <序号> <输出文件>]
//! ```

use std::time::Instant;

fn main() -> dbpf::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: inspect <package> [dump <index> <out>]");
    let (index, dump) = match args.get(2).map(String::as_str) {
        Some("dump") => (
            args.get(3).expect("dump index").parse::<usize>().unwrap(),
            Some(args.get(4).expect("dump output path")),
        ),
        _ => (usize::MAX, None),
    };

    let t0 = Instant::now();
    let package = dbpf::Package::open(path)?;
    let open_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let header = package.header();
    println!("file:          {}", package.path().display());
    println!("kind:          {:?}", header.kind);
    println!(
        "version:       {}.{}",
        header.major_version, header.minor_version
    );
    println!(
        "index:         {} entries, {} bytes @ {:#x}",
        header.index_count, header.index_size, header.index_offset
    );
    println!(
        "parsed:       {} entries in {open_ms:.1} ms",
        package.entries().len()
    );

    let entries = package.entries();
    let compressed = entries.iter().filter(|e| e.compressed).count();
    let stored_bytes: u64 = entries.iter().map(|e| e.stored_len()).sum();
    let decompressed_bytes: u64 = entries.iter().map(|e| u64::from(e.decompressed_size)).sum();
    println!(
        "compression:   {compressed}/{} entries RefPack-compressed",
        entries.len()
    );
    println!("payload:      {stored_bytes} stored / {decompressed_bytes} decompressed");

    let mut types: Vec<(u32, usize)> = {
        let mut m = std::collections::HashMap::new();
        for e in entries {
            *m.entry(e.id.type_id).or_insert(0) += 1;
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        v
    };
    types.truncate(12);
    println!("top types:");
    for (t, n) in &types {
        println!("  {t:08X}  {n}");
    }

    println!("first entries:");
    for e in entries.iter().take(index.min(entries.len())) {
        println!(
            "  {}  off={:#x} csize={} dsize={} {}",
            e.id,
            e.offset,
            e.compressed_size,
            e.decompressed_size,
            if e.compressed { "refpack" } else { "stored" },
        );
    }

    if let Some(out) = dump {
        let e = &entries[index];
        let t = Instant::now();
        let data = package.read(e)?;
        println!(
            "dump #{}: {} bytes in {:.1} ms -> {out}",
            index,
            data.len(),
            t.elapsed().as_secs_f64() * 1000.0
        );
        std::fs::write(out, &data)?;
    }

    Ok(())
}
