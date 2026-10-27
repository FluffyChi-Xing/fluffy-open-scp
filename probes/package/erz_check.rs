//! 探针：对真实 ERZ 文件运行结构解析并打印摘要。仅开发用。
//! 用法: erz_check <file.erz.bin> [...]
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("read");
        match dbpf::erz_parse_summary(&data) {
            Ok(s) => {
                println!("== {path}");
                println!(
                    "  layout={}.{} layoutRecord={} flag={} rules={} subs={} recA={} recB={} recC={} consts={} d={}/{} e={}/{} blob={} exact={}",
                    s.major, s.minor, s.layout, s.flag, s.rule_count, s.rule_name_hashes.len(),
                    s.rec_a_count, s.rec_b_count, s.rec_c_count, s.constant_count,
                    s.d_objects, s.d_entries, s.e_objects, s.e_entries, s.blob_length, s.exact
                );
                println!("  strings: {:?}", s.strings.iter().take(8).collect::<Vec<_>>());
            }
            Err(e) => println!("== {path}\n  ERR: {e}"),
        }
    }
}
