//! 探针：对比 zh-tw / en-us 语言包同表内容，确认语言分布。
use dbpf::Package;
use sc_properties::locale::Locale;

const TYPE_LOCALE: u32 = 0x0A98_EAF0;
const MENU_TABLE: u32 = 0x6C96_9DEE; // 工具名表（workbench.ts 逆向）

fn load(path: &str) -> Locale {
    let package = Package::open(path).expect("open");
    let resources = package.entries().iter().filter_map(|e| {
        if e.id.type_id != TYPE_LOCALE {
            return None;
        }
        Some((e.id.instance, package.read(e).ok()?))
    });
    Locale::from_resources(resources).expect("parse")
}

fn main() {
    for (lang, path) in [
        ("zh-tw", r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\Locale\zh-tw\Data.package"),
        ("en-us", r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\Locale\en-us\Data.package"),
    ] {
        let locale = load(path);
        // 抽样：遍历菜单表前几条字符串
        let mut shown = 0;
        println!("== {lang}: 总字符串 {} 条", locale.string_count());
        // Locale 没有 keys 迭代器——借一个已知键域扫描：直接取表内任意 3 条
        // 这里用 menu 表试几个相邻 id 段；更直接：打印该表条数与 1 条样例。
        // Locale API 有限，改用 parse_string_table 原始遍历：
        let package = Package::open(path).expect("open");
        for e in package.entries() {
            if e.id.type_id == TYPE_LOCALE && e.id.instance == MENU_TABLE {
                let data = package.read(e).unwrap();
                if let Ok(items) = sc_properties::locale::parse_locale_items(&data) {
                    for item in items.iter().filter(|i| i.id.is_some()).take(3) {
                        if shown < 3 {
                            println!("   样例: {}", item.text.chars().take(40).collect::<String>());
                            shown += 1;
                        }
                    }
                    let total = items.iter().filter(|i| i.id.is_some()).count();
                    println!("   菜单表字符串 {total} 条");
                }
                break;
            }
        }
    }
}
