//! 真实游戏包的验证测试。数据文件不入库（docs/packages 已 gitignore），
//! 文件缺失时测试直接通过，便于无数据环境跑 CI。

use std::time::Instant;

use dbpf::Package;

const REAL_PACKAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/app.package"
);

fn real_package() -> Option<Package> {
    match Package::open(REAL_PACKAGE) {
        Ok(p) => Some(p),
        Err(dbpf::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("real package failed to open: {e}"),
    }
}

#[test]
fn real_package_header_and_index() {
    let Some(package) = real_package() else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let header = package.header();
    assert_eq!(header.kind, dbpf::PackageKind::Dbpf);
    assert_eq!(header.major_version, 3);
    assert_eq!(header.index_count, 2489);
    assert_eq!(package.entries().len(), 2489);

    // SimCity_App 的已知类型分布：贴图 / RW4 模型 / 属性表
    let count_type = |t: u32| {
        package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == t)
            .count()
    };
    assert_eq!(count_type(0x2F7D_0004), 1358, "textures");
    assert_eq!(count_type(0x2F4E_681B), 538, "RW4 models");
    assert_eq!(count_type(0x00B1_B104), 249, "property lists");
}

#[test]
fn real_package_full_sweep_reads_all_resources() {
    let Some(package) = real_package() else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let t0 = Instant::now();
    let mut total = 0u64;
    let mut compressed_total = 0u64;
    for (i, e) in package.entries().iter().enumerate() {
        let data = package.read(e).unwrap_or_else(|err| {
            panic!("resource #{i} {} failed: {err}", e.id);
        });
        assert_eq!(
            data.len() as u32,
            e.decompressed_size,
            "resource #{i} {} size mismatch",
            e.id
        );
        total += data.len() as u64;
        if e.compressed {
            compressed_total += data.len() as u64;
        }
    }
    let secs = t0.elapsed().as_secs_f64();

    // 与 inspect 输出一致的总量校验
    assert_eq!(total, 481_477_425, "total decompressed bytes");
    let _ = compressed_total;
    eprintln!(
        "swept {} resources ({} decompressed) in {:.2}s",
        package.entries().len(),
        total,
        secs
    );
}
