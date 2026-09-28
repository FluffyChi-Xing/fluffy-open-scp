# lot_hires — generic_lot 引擎公式高清合成（2026-09-29）

按 `docs/overview/lot-rendering.md` §3 `generic_lot` 像素着色器算法（硬阈值 one-hot +
8 级瀑布 A>B>G>R + 每通道平色 + 边框带 + 底图格 + 图案/法线图集坡度明暗）在游戏级
尺度上合成的 lot 地表图。旧"terrain PS a⁴ 逐像素涂色 + 双线性放大"方法已证伪废弃。

## 文件

- `lot_<id>_hires_pattern.png` — **引擎口径**成品：平色 × 法线图集 `0x60E7805D`
  （格号 = LotColor.A）坡度明暗；alpha = overlayMask（未覆盖透明，即前端
  "只替换有内容区"语义）。全分辨率仅存本地（100M+，不入 git）。
- `lot_<id>_hires_tile.png` — 渲染器口径对照（漫反射平行格 × LotColor 染色），
  证明该语义双重变暗（引擎覆盖区不采样漫反射），仅留档。
- `preview/` — pattern 变体 1024px 缩略（入 git）。
- `lotm_v9_01532F56.lotm` — 市政厅族模型 0x01532F56 的 LOTM v9 容器
  （1 mesh / 1 material / 6 slot：paletteF32 参数表 + 3×raster + rawBGRA + DXT5），
  由 `cargo test -p fluffy-open-scp --release --lib dump_lotm_v9_city_hall` 生成。
- `lotm_v9_01532F56_diag.txt` — 槽位诊断文本。

## 覆盖的 lot

| lot | 模型/备注 | LotSize (m) | mask | baseTile 来源 | A索引 |
|---|---|---|---|---|---|
| `457EA9DB` / `909BD1C8` / `909BD1DB` | 市政厅族（LOD1 = 0x01532F56） | 192×96 | `ee9e76d3` 256×128 | FD2/FD3 推导 = 4 | 3/1/8/3 |
| `65A873B3` / `65A873A0` | 广场（圆形+六角） | 72×72 | `205042ec` 128² | FD6 = 1 | 4/2/1/6 |
| `7B20A320` / `7B20A333` / `C2AB4548` / `C2AB455B` | 大广场 | 216×216 | `487dc37c` 256² | FD6 = 1 | 1/1/2/5 |
| `FA8C9AA7` / `99ACC0AF` | 圆厅 | 96×96 | `fa8c9aa7` 128² | FD6 = 1 | 9/2/1/3 |
| `B2B62C0B` | 超宽 lot | 386×192 | `6f08cbf7` 256×128 | FD6 = 4 | 8/2/1/3 |

## 再生成

```bash
# 单 lot 高清（--ppm 默认 32，超 4096 等比缩）
cargo run -p sc-exporter --release --example lot_composite -- \
  --lot=457ea9db --out=output/lot_hires

# 批量原生分辨率 albedo/selector（contact sheet）
cargo run -p sc-exporter --release --example lot_composite

# LOTM v9 容器落盘
cargo test -p fluffy-open-scp --release --lib dump_lotm_v9_city_hall -- --nocapture
```

## 对渲染器的两个已证结论

1. **左右镜像**：本目录合成按引擎约定做了 mask 180° 旋转（§5b）；OpenSCP 现行
   `refinedGround` 未做列镜像 → 与游戏/本目录输出整体左右镜像（用户截图对拍吻合）。
2. **覆盖区语义**：引擎反照率 = 通道平色（质感来自法线图案光照），不是
   "tile × tint"——后者见 `*_hires_tile.png` 的双重变暗。
