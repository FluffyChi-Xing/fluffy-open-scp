//! 0x0469A3F7 shader 容器无损解析（parse_container_tokens_full.py 的 Rust 移植）。
//!
//! 容器 = 扁平 `[u8 len][text]` 串流，单 token 上限 250B；>250B 的片段被切成
//! 多个连续 token。旧解析器只保留最长单 token 丢失其余切片（decal 族分析
//! 被截断污染的根因）；本实现按「块名 + 连续 body 串流」无损重组。

/// 单个命名片段（引擎组合系统的最小单元）。
#[derive(Debug, Clone)]
pub struct Fragment {
    pub name: String,
    pub body: String,
}

/// 标识符样 token（块名候选）：无空格、驼峰/下划线。
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    let len = 1 + chars.filter(|c| c.is_ascii_alphanumeric() || *c == '_').count();
    (2..=61).contains(&len) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 源码样 token（块体候选）。
fn is_source(s: &str) -> bool {
    if is_ident(s) {
        return false;
    }
    ["= ", "(", ";", "float", "return", "clip", "mul("]
        .iter()
        .any(|h| s.contains(h))
}

/// 片段边界判据：chunk 是否以自然终止符收尾（决定后续标识符是断词续片还是新块名）。
fn ends_terminated(chunk: &str) -> bool {
    let t = chunk.trim_end();
    t.is_empty() || t.ends_with([';', '}', '\n', '\r'])
}

fn tokens(data: &[u8]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let l = data[i] as usize;
        if (4..=250).contains(&l) && i + 1 + l <= data.len() {
            let s = &data[i + 1..i + 1 + l];
            if s.iter().all(|&c| (0x20..0x7f).contains(&c) || c == b'\t' || c == b'\n' || c == b'\r') {
                out.push(std::str::from_utf8(s).unwrap());
                i += 1 + l;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 解析一个容器 bin，返回去重（同名取最长）的片段表。
pub fn parse(data: &[u8]) -> Vec<Fragment> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut pending_name: Option<String> = None;
    let mut pending_body = String::new();

    for tok in tokens(data) {
        if is_source(tok) {
            if pending_name.is_some() {
                pending_body.push_str(tok);
            }
        } else if is_ident(tok) {
            if !pending_body.is_empty() && !ends_terminated(&pending_body) {
                // 断词续片：上片没收尾，此「标识符」其实是代码断片
                pending_body.push_str(tok);
            } else {
                if let (Some(name), false) = (&pending_name, pending_body.is_empty()) {
                    upsert(&mut blocks, name.clone(), pending_body.clone());
                }
                pending_name = Some(tok.to_string());
                pending_body.clear();
            }
        }
    }
    if let (Some(name), false) = (&pending_name, pending_body.is_empty()) {
        upsert(&mut blocks, name.clone(), pending_body);
    }
    blocks.into_iter().map(|(name, body)| Fragment { name, body }).collect()
}

fn upsert(blocks: &mut Vec<(String, String)>, name: String, body: String) {
    if let Some(e) = blocks.iter_mut().find(|(n, _)| *n == name) {
        if body.len() > e.1.len() {
            e.1 = body;
        }
    } else {
        blocks.push((name, body));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_container(frags: &[(&str, &str)]) -> Vec<u8> {
        // 模拟引擎串流：>250B 片段切成 250B chunks
        let mut out = Vec::new();
        for (name, body) in frags {
            for c in name.as_bytes().chunks(250) {
                out.push(c.len() as u8);
                out.extend_from_slice(c);
            }
            for c in body.as_bytes().chunks(250) {
                out.push(c.len() as u8);
                out.extend_from_slice(c);
            }
        }
        out
    }

    #[test]
    fn joins_chunks_over_250b() {
        let long_body = format!("float x = {};{}", "1.0;".repeat(80), "y = 2;");
        let data = build_container(&[("decalTest", &long_body)]);
        let frags = parse(&data);
        let f = frags.iter().find(|f| f.name == "decalTest").unwrap();
        // 全部字节可打印 → 无损重组（边界零损耗）
        assert_eq!(f.body, long_body);
    }

    #[test]
    fn ident_after_terminated_chunk_is_new_block() {
        let data = build_container(&[("alpha", "x = 1;"), ("gamma", "y = 2;")]);
        let frags = parse(&data);
        assert_eq!(frags.len(), 2);
        assert!(frags.iter().any(|f| f.name == "gamma"));
    }
}
