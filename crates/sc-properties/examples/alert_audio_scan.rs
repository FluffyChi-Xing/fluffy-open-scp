//! 探针：验证警报 property(0x098A44F2) 是否关联可播放音频。仅开发用。
//!
//! 结论（2026-09-19 实证）：否。
//! - Game 包 0x0D9E5710 (wav audio) 资源数 = 0（本探针输出 total）；
//! - 警报声音走 Wwise：SimCity_Audio_Banks.package 仅含 83 个
//!   0x0A4D8D09 (Wwise SoundBank)，无独立 wav；
//! - property 内 0x0FC45F4F Key → 922CC740 不落在任何包资源上
//!   （全包 instance 搜索为空），是模拟侧/Wwise 事件 id，非音频 TGI；
//! - float 5（0x098BD066）= 警报横幅展示时长（秒），非音频长度。
use dbpf::Package;
use sc_registry::Registry;

fn main() {
    let package = Package::open(
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
    )
    .expect("open");
    let registry = Registry::open(r"docs\packages\database_main.s3db").expect("registry");
    let mut total = 0usize;
    let mut named = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x0D9E_5710 {
            continue;
        }
        total += 1;
        let name = registry
            .instances()
            .get(&entry.id.instance)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        let lower = name.to_lowercase();
        if !name.is_empty() {
            named += 1;
        }
        if ["alert", "educat", "school", "siren", "bell"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            println!(
                "HIT {:08X} group={:08X} name={}",
                entry.id.instance, entry.id.group, name
            );
        }
    }
    println!("total wav={total}, named={named}");
}
