//! 真实描述符库验证（database_main.s3db，不入库；缺失时跳过）。

use sc_registry::Registry;

const MAIN_DB: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/packages/database_main.s3db");

#[test]
fn loads_descriptor_tables() {
    if !std::path::Path::new(MAIN_DB).exists() {
        eprintln!("skipping: {MAIN_DB} not present");
        return;
    }

    let registry = Registry::open(MAIN_DB).unwrap();

    // Properties 表应包含 Model Details（HANDOFF §4：描述符库中恰好一条）
    let model_details = 0x0975_695F;
    let prop = registry.properties().get(&model_details).expect("Model Details descriptor missing");
    assert!(prop.name.contains("Model Details"), "got {prop:?}");

    // 实例/类型表应有可观的数据量
    assert!(
        registry.instances().len() > 100,
        "instances table suspiciously small: {}",
        registry.instances().len()
    );
    assert!(
        registry.properties().len() > 100,
        "properties table suspiciously small: {}",
        registry.properties().len()
    );

    // RW4 模型类型 id 应能解析出名称（非 hex 回退）
    let rw4 = registry.type_name(0x2F4E_681B);
    assert_ne!(rw4, "2F4E681B", "RW4 model type should resolve to a name, got hex fallback");
    eprintln!("0x2F4E681B -> {rw4:?}");

    // 名称回退：不存在的 id 返回 hex
    assert_eq!(registry.instance_name(0xDEAD_BEEF), "DEADBEEF");
}
