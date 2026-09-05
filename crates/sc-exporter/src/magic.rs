//! 二进制魔数（magic number）撞库识别 —— 非阻塞纯函数。
//!
//! 用途：前端遇到 unknown 资源时，把资源头部（前 16 字节足够）交给
//! [`detect_magic`] 撞库。命中 → 返回格式元数据（前端提示
//! 「检测到 .xx 文件，暂不支持预览」）；未命中 → 按原未知文件处理。
//!
//! 非阻塞保证：纯内存前缀匹配，无 IO、无分配、O(规则数) 常数极小，
//! 可安全用于 Tauri command 直接调用（无需 spawn_blocking）。
//!
//! 覆盖两类来源：
//! 1. 通用游戏资源魔数（ZIP/GZIP/Zlib/PNG/JPEG/DDS/RIFF/OggS/ICO/XML…）
//! 2. SCP 生态特有（EA DBPF 容器 .package、SQLite s3db、TTF/OTF 字体等）

/// 一条魔数规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicFormat {
    /// 头部 hex（展示用，如 `"50 4B 03 04"`）。
    pub hex: &'static str,
    /// ASCII 可视化（如 `"PK.."`，不可见字符用 `..`）。
    pub ascii: &'static str,
    /// 建议扩展名（不含点）。
    pub extension: &'static str,
    /// 极有可能的格式。
    pub format: &'static str,
    /// 常见游戏用途。
    pub usage: &'static str,
    /// 文件头前缀字节。
    pub prefix: &'static [u8],
}

/// 规则库。`prefix` 更长者先匹配（防 `1F 8B` 与更长前缀误撞）。
pub const MAGIC_FORMATS: &[MagicFormat] = &[
    MagicFormat {
        hex: "50 4B 03 04",
        ascii: "PK..",
        extension: "zip",
        format: "ZIP（或 Unity .assetbundle）",
        usage: "Unity 游戏资源包、Android 数据包",
        prefix: b"PK\x03\x04",
    },
    MagicFormat {
        hex: "44 42 50 46",
        ascii: "DBPF",
        extension: "package",
        format: "EA DBPF 容器",
        usage: "SimCity/EA .package 资源包（本项目主格式）",
        prefix: b"DBPF",
    },
    MagicFormat {
        hex: "89 50 4E 47",
        ascii: ".PNG",
        extension: "png",
        format: "PNG 图片",
        usage: "贴图、UI 图标",
        prefix: b"\x89PNG",
    },
    MagicFormat {
        hex: "FF D8 FF",
        ascii: "(不可见)",
        extension: "jpg",
        format: "JPEG 图片",
        usage: "过场 CG 或高质量背景",
        prefix: b"\xFF\xD8\xFF",
    },
    MagicFormat {
        hex: "44 44 53 20",
        ascii: "DDS ",
        extension: "dds",
        format: "DDS 纹理（DirectDraw Surface）",
        usage: "直接驻留显存的显卡纹理格式",
        prefix: b"DDS ",
    },
    MagicFormat {
        hex: "4F 67 67 53",
        ascii: "OggS",
        extension: "ogg",
        format: "OGG 音频（Vorbis/Opus）",
        usage: "背景音乐、语音包",
        prefix: b"OggS",
    },
    MagicFormat {
        hex: "37 7A BC AF 27 1C",
        ascii: "7z....'C",
        extension: "7z",
        format: "7-Zip 压缩包",
        usage: "打包分发的模组压缩包",
        prefix: b"7z\xBC\xAF\x27\x1C",
    },
    MagicFormat {
        hex: "52 61 72 21",
        ascii: "Rar!",
        extension: "rar",
        format: "RAR 压缩包",
        usage: "打包分发的模组压缩包",
        prefix: b"Rar!",
    },
    MagicFormat {
        hex: "53 51 4C 69 74 65 20 66 6F 72 6D 61 74 20 33",
        ascii: "SQLite format 3",
        extension: "s3db",
        format: "SQLite 3 数据库",
        usage: "SCP 描述符库 database_main/user.s3db",
        prefix: b"SQLite format 3\x00",
    },
    MagicFormat {
        hex: "55 6E 69 74 79 46 53",
        ascii: "UnityFS",
        extension: "unity3d",
        format: "Unity Asset Bundle",
        usage: "Unity 引擎游戏资源包",
        prefix: b"UnityFS",
    },
    MagicFormat {
        hex: "1F 8B",
        ascii: "(不可见)",
        extension: "gz",
        format: "GZIP",
        usage: "压缩的纹理或关卡数据",
        prefix: b"\x1F\x8B",
    },
    MagicFormat {
        hex: "3C 3F 78 6D 6C",
        ascii: "<?xml",
        extension: "xml",
        format: "XML 文本（明文配置）",
        usage: "剧情文本、数值表",
        prefix: b"<?xml",
    },
    // ICO：`00 00 01 00`；前缀太短易误撞，要求保留字段 0 保持 0
    MagicFormat {
        hex: "00 00 01 00",
        ascii: "(不可见)",
        extension: "ico",
        format: "ICO 图标 或 Cursor 光标",
        usage: "游戏内鼠标指针（SCP Cursor 资源同源）",
        prefix: &[0x00, 0x00, 0x01, 0x00],
    },
    MagicFormat {
        hex: "00 01 00 00",
        ascii: "(不可见)",
        extension: "ttf",
        format: "TrueType 字体",
        usage: "游戏 UI 字体（SCP viewData 字体资源同源）",
        prefix: &[0x00, 0x01, 0x00, 0x00],
    },
    MagicFormat {
        hex: "4F 54 54 4F",
        ascii: "OTTO",
        extension: "otf",
        format: "OpenType 字体",
        usage: "游戏 UI 字体",
        prefix: b"OTTO",
    },
    MagicFormat {
        hex: "42 4D",
        ascii: "BM",
        extension: "bmp",
        format: "Windows 位图",
        usage: "简单贴图、启动图",
        prefix: b"BM",
    },
    // 78 9C / 78 DA：zlib（Deflate）。放最后——2 字节前缀置信度最低
    MagicFormat {
        hex: "78 9C / 78 DA",
        ascii: "(不可见)",
        extension: "zlib",
        format: "Zlib（Deflate 压缩）",
        usage: "很多引擎的通用压缩块",
        prefix: &[0x78, 0x9C],
    },
];

/// RIFF 容器细分：offset 8..12 决定 WAV / WebP / AVI。
const RIFF_WAVE: MagicFormat = MagicFormat {
    hex: "52 49 46 46 .. .. .. .. 57 41 56 45",
    ascii: "RIFF....WAVE",
    extension: "wav",
    format: "WAV 音频",
    usage: "音效（SCP viewData 的 Wav Audio 同源）",
    prefix: b"RIFF",
};
const RIFF_WEBP: MagicFormat = MagicFormat {
    hex: "52 49 46 46 .. .. .. .. 57 45 42 50",
    ascii: "RIFF....WEBP",
    extension: "webp",
    format: "WebP 图片",
    usage: "图片资源",
    prefix: b"RIFF",
};
const RIFF_AVI: MagicFormat = MagicFormat {
    hex: "52 49 46 46 .. .. .. .. 41 56 49 20",
    ascii: "RIFF....AVI ",
    extension: "avi",
    format: "AVI 视频",
    usage: "过场视频容器",
    prefix: b"RIFF",
};
/// 兜底 RIFF（无法细分）。
const RIFF_GENERIC: MagicFormat = MagicFormat {
    hex: "52 49 46 46",
    ascii: "RIFF",
    extension: "riff",
    format: "RIFF 容器（WAV 音频 或 WebP 图片）",
    usage: "音效或图片",
    prefix: b"RIFF",
};

/// 撞库识别：返回命中的规则；未命中返回 `None`（调用方按原未知文件处理）。
///
/// `bytes` 只需资源头部（建议 ≥16 字节）。
pub fn detect_magic(bytes: &[u8]) -> Option<&'static MagicFormat> {
    if bytes.is_empty() {
        return None;
    }

    // RIFF 细分优先
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" {
        let kind = &bytes[8..12];
        return Some(match kind {
            b"WAVE" => &RIFF_WAVE,
            b"WEBP" => &RIFF_WEBP,
            b"AVI " => &RIFF_AVI,
            _ => &RIFF_GENERIC,
        });
    }

    // zlib 两个签名
    if bytes.len() >= 2 && bytes[0] == 0x78 && matches!(bytes[1], 0x9C | 0xDA | 0x01 | 0x5E) {
        return MAGIC_FORMATS.iter().find(|f| f.extension == "zlib");
    }

    // 其余按前缀最长优先
    let mut best: Option<&'static MagicFormat> = None;
    for rule in MAGIC_FORMATS {
        if bytes.starts_with(rule.prefix) && best.is_none_or(|b| rule.prefix.len() > b.prefix.len())
        {
            best = Some(rule);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_game_magics() {
        assert_eq!(detect_magic(b"PK\x03\x04rest").unwrap().extension, "zip");
        assert_eq!(detect_magic(b"\x89PNG\r\n\x1a\n").unwrap().extension, "png");
        assert_eq!(detect_magic(b"\xFF\xD8\xFF\xE0").unwrap().extension, "jpg");
        assert_eq!(detect_magic(b"DDS |\x00\x00\x00").unwrap().extension, "dds");
        assert_eq!(detect_magic(b"OggS\x00\x02").unwrap().extension, "ogg");
        assert_eq!(detect_magic(b"DBPF\x0C\x00").unwrap().extension, "package");
        assert_eq!(
            detect_magic(b"UnityFS\x00\x00").unwrap().extension,
            "unity3d"
        );
        assert_eq!(
            detect_magic(b"<?xml version=\"1.0\"?>").unwrap().extension,
            "xml"
        );
        let gz: &[u8] = &[0x1F, 0x8B, 0x08, 0x00];
        assert_eq!(detect_magic(gz).unwrap().extension, "gz");
        let zlib: &[u8] = &[0x78, 0xDA, 0x01];
        assert_eq!(detect_magic(zlib).unwrap().extension, "zlib");
    }

    #[test]
    fn detects_riff_variants() {
        let wav = b"RIFF\x24\x00\x00\x00WAVEfmt ";
        assert_eq!(detect_magic(wav).unwrap().extension, "wav");
        let webp = b"RIFF\x24\x00\x00\x00WEBPVP8 ";
        assert_eq!(detect_magic(webp).unwrap().extension, "webp");
        let avi = b"RIFF\x24\x00\x00\x00AVI LIST";
        assert_eq!(detect_magic(avi).unwrap().extension, "avi");
        let bare = b"RIFF\x24\x00\x00\x00????";
        assert_eq!(detect_magic(bare).unwrap().extension, "riff");
    }

    #[test]
    fn ico_ttf_short_prefixes() {
        let ico: &[u8] = &[0x00, 0x00, 0x01, 0x00, 0x02, 0x00];
        assert_eq!(detect_magic(ico).unwrap().extension, "ico");
        let ttf: &[u8] = &[0x00, 0x01, 0x00, 0x00, 0x00];
        assert_eq!(detect_magic(ttf).unwrap().extension, "ttf");
        let otf = b"OTTO\x00\x01";
        assert_eq!(detect_magic(otf).unwrap().extension, "otf");
    }

    #[test]
    fn unknown_inputs_return_none() {
        assert_eq!(detect_magic(b""), None);
        assert_eq!(detect_magic(b"\x00\x01\x02"), None);
        assert_eq!(detect_magic(b"just some plain ascii text"), None);
        // 78 后跟非 zlib CLEVEL：78 20 不在允许集
        assert_eq!(detect_magic(&[0x78, 0x20]), None);
    }

    #[test]
    fn sqlite_and_fonts_cover_scp_ecosystem() {
        assert_eq!(
            detect_magic(b"SQLite format 3\x00").unwrap().extension,
            "s3db"
        );
    }
}
