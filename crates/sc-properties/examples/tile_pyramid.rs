//! tile 金字塔匹配探针：验证 341 = mip0(256)+mip1(64)+mip2(16)+mip3(4)+mip4(1)。
//! 父 tile 每象限 = 子 tile 的 2×2 降采样（box 或点采样，两种都试）。
//! 用法：tile_pyramid <package> <region-group>
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let data = package.read(e).expect("read");
            let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            if px.len() == 65536 { tiles.insert(e.id.instance, px); }
        }
    }
    println!("tiles: {}", tiles.len());

    // 预计算每 tile 的 2×2 box 降采样（128×128）与点采样（even,even）
    let mut ds_box: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut ds_pt: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&inst, px) in &tiles {
        let mut b = vec![0u32; 128 * 128];
        let mut p = vec![0u32; 128 * 128];
        for y in 0..128 {
            for x in 0..128 {
                let (X, Y) = (2 * x, 2 * y);
                let i = Y * 256 + X;
                b[y * 128 + x] = (u32::from(px[i]) + u32::from(px[i + 1]) + u32::from(px[i + 256]) + u32::from(px[i + 257])) / 4;
                p[y * 128 + x] = u32::from(px[i]);
            }
        }
        ds_box.insert(inst, b);
        ds_pt.insert(inst, p);
    }

    // 对每对 (child, parent)：找 parent 四象限中与 child 降采样平均绝对差最小的象限
    // 输出显著匹配（avg|Δ| < 阈值）。两倍采样率下若假设成立应精确。
    let insts: Vec<u32> = tiles.keys().copied().collect();
    let mut matches: Vec<(u32, u32, usize, f64, f64)> = Vec::new(); // child,parent,quadrant,errbox,errpt
    for &child in &insts {
        let cb = &ds_box[&child];
        let cp = &ds_pt[&child];
        let mut best: Vec<(u32, usize, f64, f64)> = Vec::new();
        for &parent in &insts {
            if parent == child { continue; }
            let px = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 128, (q / 2) * 128);
                let mut eb = 0f64; let mut ep = 0f64;
                for y in 0..128 {
                    let prow = (qy + y) * 256 + qx;
                    let crow = y * 128;
                    for x in 0..128 {
                        let pv = px[prow + x] as f64;
                        eb += (pv - cb[crow + x] as f64).abs();
                        ep += (pv - cp[crow + x] as f64).abs();
                    }
                }
                eb /= 16384.0; ep /= 16384.0;
                best.push((parent, q, eb, ep));
            }
        }
        best.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        let b0 = best[0];
        println!("child 0x{child:08X}: best parent 0x{:08X} q{} boxerr={:.2} pterr={:.2} | 2nd 0x{:08X} boxerr={:.2}",
            b0.0, b0.1, b0.2, b0.3, best[1].0, best[1].2);
        matches.push((child, b0.0, b0.1, b0.2, b0.3));
    }
    // 汇总：如果假设成立，64 个 parent 各应有 4 个 child（象限各一），误差接近 0
    let good = matches.iter().filter(|m| m.3 < 40.0).count();
    println!("== matches with boxerr<40: {good} / {}", matches.len());
}
