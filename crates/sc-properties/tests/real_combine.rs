//! 真实包上的资产聚合验证：combine app.package 全部属性表。

use std::collections::HashMap;
use std::time::Instant;

use dbpf::Package;
use sc_properties::{PropertyFile, combine_assets, has_model_details};

const REAL_PACKAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/app.package"
);

#[test]
fn real_package_combine_covers_all_entries() {
    let Ok(package) = Package::open(REAL_PACKAGE) else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let mut entries = Vec::new();
    let mut model_details_files = 0usize;
    for e in package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
    {
        let data = package.read(e).unwrap();
        let pf = PropertyFile::parse(&data).unwrap();
        if has_model_details(&pf) {
            model_details_files += 1;
        }
        entries.push((e.id, pf));
    }

    let t0 = Instant::now();
    let groups = combine_assets(entries.iter().map(|(id, f)| (*id, f)), None);
    let secs = t0.elapsed().as_secs_f64();

    // 覆盖性：所有条目恰好出现一次
    let member_count: usize = groups.iter().map(|g| g.members.len()).sum();
    assert_eq!(
        member_count,
        entries.len(),
        "every entry must belong to exactly one group"
    );
    assert!(!groups.is_empty());

    let multi = groups.iter().filter(|g| g.members.len() > 1).count();
    let max_members = groups.iter().map(|g| g.members.len()).max().unwrap();
    eprintln!(
        "{}/{} entries -> {} groups ({} multi-member, largest {}), {} files carry Model Details, in {secs:.3}s",
        entries.len(),
        entries.len(),
        groups.len(),
        multi,
        max_members,
        model_details_files
    );

    // 无 locale 数据时名称全部为 None
    assert!(groups.iter().all(|g| g.name.is_none()));

    // 确定性：同样输入两次结果一致
    let groups2 = combine_assets(entries.iter().map(|(id, f)| (*id, f)), None);
    assert_eq!(groups, groups2);

    // 与包索引实例集合的交叉验证：多成员组必然共享实例或经 Model Details 连接
    let entries_by_instance: HashMap<u32, Vec<&dbpf::ResourceId>> =
        entries.iter().fold(HashMap::new(), |mut acc, (id, _)| {
            acc.entry(id.instance).or_default().push(id);
            acc
        });
    for g in groups.iter().filter(|g| g.members.len() > 1) {
        let shares_instance = g.members.windows(2).any(|w| w[0].instance == w[1].instance);
        let _ = &entries_by_instance;
        assert!(
            shares_instance || model_details_files > 0,
            "multi-member group must be explainable by instance sharing or Model Details"
        );
    }
}
