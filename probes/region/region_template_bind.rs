//! 探针：区域模板 JSON（EP1 包 4529F96F）↔ RT0 地块表配对验证 + 伟工位输出。仅开发用。
use std::collections::HashSet;

fn main() {
    let ep1 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package").unwrap();
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    // RT0: region group → 表内城市 id 集
    let mut city_region: std::collections::HashMap<u32, u32> = Default::default();
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 { continue; }
        let Ok(pf) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) else { continue };
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
            if k.instance == 0x51E7_A18D { city_region.insert(e.id.group, k.group); }
        }
    }
    let mut tables: Vec<(u32, Vec<String>)> = Vec::new();
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C { continue; }
        let Ok(pt) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) else { continue };
        let names: Vec<String> = match pt.get(0x0543_BC96) {
            Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) =>
                vs.iter().filter_map(|v| match v { sc_properties::Value::String8(s) => Some(s.clone()), _ => None }).collect(),
            _ => continue,
        };
        tables.push((e.id.group, names));
    }
    let mut region_cities: std::collections::HashMap<u32, Vec<u32>> = Default::default();
    for (c, r) in &city_region { region_cities.entry(*r).or_default().push(*c); }

    // EP1: 模板 JSON → 城市 uid 集
    for e in ep1.entries() {
        if e.id.type_id != 0x0A98_EAF0 || e.id.instance != 0x4529_F96F { continue; }
        let Ok(d) = ep1.read(e) else { continue };
        let Ok(txt) = std::str::from_utf8(&d) else { continue };
        let txt = txt.trim_start_matches(|c: char| c != '{');
        let v = match json::parse(txt) { Ok(v) => v, Err(e) => { println!("parse 失败 {e:?} 前80字节: {:?}", &txt[..txt.len().min(80)]); continue } };
        let get = |o: &json::Value, k: &str| o.get(k).map(|x| x.as_str().unwrap_or("")).unwrap_or("").to_string();
        let name = get(&v, "name");
        let cities: Vec<json::Value> = v.get("map").and_then(|m| m.get("cities")).and_then(|c| c.as_array()).map(|a| a.to_vec()).unwrap_or_default();
        let uids: Vec<String> = cities.iter().map(|c| get(c, "uid")).collect();
        // 匹配 RT0 区域：地块表名字串交集
        let set: HashSet<String> = uids.iter().cloned().collect();
        let mut best: Option<(usize, u32)> = None;
        for (r, cs) in &region_cities {
            let cs_set: HashSet<&u32> = cs.iter().collect();
            let _ = cs_set;
            let _ = r;
            break;
        }
        // 直接对地块表名字串匹配
        let mut best2: Option<(usize, u32, usize)> = None;
        for (tg, names) in &tables {
            let ov = names.iter().filter(|n| set.contains(*n)).count();
            if best2.as_ref().map(|(b, _, _)| ov > *b).unwrap_or(true) { best2 = Some((ov, *tg, names.len())); }
        }
        let (ov, tg, tl) = best2.unwrap_or((0, 0, 0));
        let _ = &best;
        // 表 group → 区域 group（该表与哪个区域的城市组交集最大，沿用 region_bind 逻辑略）：
        // 这里直接输出表 group，区域名由用户对照
        let region = tg;
        let gw: Vec<json::Value> = v.get("map").and_then(|m| m.get("greatWorks")).and_then(|c| c.as_array()).map(|a| a.to_vec()).unwrap_or_default();
        let gw_pos: Vec<String> = gw.iter().map(|g| format!("({:.0},{:.0})", g.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0), g.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0))).collect();
        println!("group={:08X} {:14} 表={:08X} 城市uid {}/{} 伟工{}处 {:?}", e.id.group, name, region, uids.len(), ov, tl, gw_pos);
    }
}

/// 极简 JSON 解析（模板为规整的机器生成 JSON，够用）。
mod json {
    pub fn parse(s: &str) -> Result<Value, String> { let mut p = P { b: s.as_bytes(), i: 0 }; p.ws(); p.val() }
    #[derive(Clone)]
    pub enum Value { S(String), N(f64), B(bool), A(Vec<Value>), O(Vec<(String, Value)>) }
    impl Value {
        pub fn get(&self, k: &str) -> Option<&Value> { if let Value::O(m) = self { m.iter().find(|(kk, _)| kk == k).map(|(_, v)| v) } else { None } }
        pub fn as_array(&self) -> Option<&Vec<Value>> { if let Value::A(a) = self { Some(a) } else { None } }
        pub fn as_str(&self) -> Option<&str> { if let Value::S(s) = self { Some(s) } else { None } }
        pub fn as_f64(&self) -> Option<f64> { if let Value::N(n) = self { Some(*n) } else { None } }
    }
    struct P<'a> { b: &'a [u8], i: usize }
    impl<'a> P<'a> {
        fn ws(&mut self) { while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() { self.i += 1 } }
        fn val(&mut self) -> Result<Value, String> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => { self.i += 1; let mut m = Vec::new(); loop { self.ws(); if self.b.get(self.i) == Some(&b'}') { self.i += 1; break } let k = self.str_()?; self.ws(); self.i += 1; let v = self.val()?; m.push((k, v)); self.ws(); if self.b.get(self.i) == Some(&b',') { self.i += 1 } } Ok(Value::O(m)) }
                Some(b'[') => { self.i += 1; let mut a = Vec::new(); loop { self.ws(); if self.b.get(self.i) == Some(&b']') { self.i += 1; break } a.push(self.val()?); self.ws(); if self.b.get(self.i) == Some(&b',') { self.i += 1 } } Ok(Value::A(a)) }
                Some(b'"') => Ok(Value::S(self.str_()?)),
                Some(_) => { let s = self.i; while self.i < self.b.len() && !matches!(self.b[self.i], b',' | b'}' | b']' | b'\n' | b' ') { self.i += 1 } let t = std::str::from_utf8(&self.b[s..self.i]).map_err(|e| e.to_string())?.trim(); Ok(if t == "true" { Value::B(true) } else if t == "false" { Value::B(false) } else { t.parse().map(Value::N).map_err(|e: _| format!("{e} @i={} token={t:?}", self.i))? }) }
                None => Err("eof".into()),
            }
        }
        fn str_(&mut self) -> Result<String, String> {
            while self.b.get(self.i) != Some(&b'"') { self.i += 1 }
            self.i += 1;
            let s = self.i;
            while self.b[self.i] != b'"' { self.i += 1 }
            self.i += 1;
            Ok(String::from_utf8_lossy(&self.b[s..self.i - 1]).into_owned())
        }
    }
}
