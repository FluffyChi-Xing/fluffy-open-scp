> **总入口**：`docs/re/engine-contracts-and-port-status-20261111.md`。本文为模型侧逐函数缺口清单。

# SC5 Asset Packager 0.8.0 模型/材质链路 → Rust `asset_build_m1` 逐函数比对

状态：只读调查结论（2026-11-11）。未修改任何 Rust/Python 代码，未运行任何构建。
判据：用户要求"1:1 原样迁移"，**任何语义偏差都记为缺口**，即使偏差方向是"更严格"或"更安全"。

---

## 0. 基线身份（先验哈希，符合 AGENTS.md 参照纪律）

| 用途 | 路径 | 与 `source-manifest.json` 核对 |
|---|---|---|
| 任务指定基准 | `D:/rust/packages/fluffy-open-scp/reference/sc5-packager-0.8.0/` | **不完整**：110 条清单缺 57 条，其中 `engine/` 全部 23 个文件、`prop_assets/` 11 个、`vendor_tk/` 等 23 个全缺。该副本只有根目录 `.py`，**没有** `engine/`（`prepare_asset.py`/`native_controls.py`/`build_candidate.py`/`rw4_bridge.py` 等模型材质链路本体） |
| 实际采用的权威基准 | `D:/rust/packages/fluffy-open-scp/investigation/sc5-asset-packager-0.8.0/` | **110/110 全命中，0 差异**（sha256 逐文件核对） |

> 后续所有 "Python file:line" 均指 `investigation/sc5-asset-packager-0.8.0/`。`reference/` 副本不足以做模型/材质比对，建议补全或删除以免再次误用（这是本次调查本身的第一个缺口，属于流程层）。

二进制模板也做了哈希核对（Rust `src-tauri/resources/sc5/` vs Python `engine/`）：

| 文件 | sha256 前缀 | 结论 |
|---|---|---|
| `office-room.png` | `7fd50bbc…` | 一致 |
| `DCBEA56D.texture-template.rw4` | `0c966959…` | 一致 |
| `2EF7D20A.texture-template.rw4` | `0232c2ab…` | 一致 |
| `175EFB4E.research-template.rw4` | `105674c4…` | 一致 |
| `CD6F8697.collision-source.rw4`（Rust 名 `collision-source.rw4`） | `af00aff6…` | 一致 |

---

## 1. 分级汇总

| 级别 | 条数 | 其中影响外观 | 说明 |
|---|---|---|---|
| 必须修 | 5 | 5 | 全部落在渲染/外观或静默丢数据 |
| 建议修 | 17 | 2（菜单大图、面元单元 relief gain） | 校验缺失、容错差异、字节级细节；另 S9/S23 为"部分缺失/方向相反"的混合项 |
| 等价 / 等价（有意偏差） | 25 组（E1–E18、S2/S5/S6/S13/S19/S24/S25） | — | 已逐字比对，列入第 5 节避免重复排查 |

结论一句话：**room_strip / 六槽材质 / 几何核心算法是逐字等价的高质量移植**；风险集中在"构建参数没有被传递"（nightEmissionPolicy、compact_2048、默认旋转）与"Python 的构建后回读校验没有对应物"。

---

## 2. 缺口总表

编号规则：M=必须修，S=建议修，E=等价（第 5 节）。"影响"一列只在有渲染/存档含义时写实机后果。

### Q1 room_strip / 16 态内景

| # | Python | Rust | 差异 | 影响 | 级别 |
|---|---|---|---|---|---|
| M1 | `asset_transforms.py:29-50 room_strip`；调用点 `prepare_asset.py:35`（`night_only=night_only and p[0,2]>0`）、`backend_compile.py:28-31,118`（`nightEmissionPolicy`：`night-only`/`preserve-source`） | `single_compatible.rs:464-507 render_room_strip`；`author_materials.rs:74-96 compile_materials(doc, night_only)`；调用点 **`package.rs:482` 硬编码 `compile_materials(&doc, true)`** | `preserve-source` 策略完全没有实现：设置项未进入 Rust 构建（schema/plan 里不存在该字段），构建恒为 night-only | 选择 `preserve-source` 的项目，白天/关闭态窗格被强制清零 ⇒ 与 Packager 产出外观不同；夜景不受影响 | 必须修 |
| S1 | `asset_transforms.py:46-49` 逐 state 产出 `alphaBefore/alphaAfter/rgbUnchanged/nightUnchanged` 证据，`prepare_asset.py:42` 落进 `材质采样变换.json`，`backend_compile.py:247-258` 回读比对 | `render_room_strip` 只累计 `sampled_alpha/output_night_alpha` 两个计数器，不产出任何证据 JSON | 无 room-state 证据链 | 只能靠肉眼进游戏发现窗格全黑；排查成本高 | 建议修 |
| S2 | `asset_transforms.py:43` 只对 `state%4 != 1` 清零；`audit_window_night.py:32-49` 断言非夜景全零、夜景不变、RGB 不变、全透明源保持全暗 | `single_compatible.rs:491-505`：清零逻辑相同；额外门 `if apply_night_only && sampled_alpha>0 && output_night_alpha==0 → Err` | Rust 多一个"采样有发光但夜景列全空"的拒绝门 | 对 Python 能成功构建的输入（发光只在偶数列）Rust 会失败。属**有意 fail-closed**，需登记为已知偏差 | 等价（有意偏差） |
| S3 | `asset_transforms.py:35-37` `np.mod` + `floor`，索引越界会抛 `IndexError` | `single_compatible.rs:475-481` `rem_euclid` + `floor` + `as u32`，越界会 **panic**（`get_pixel`） | 极端浮点输入（`mod` 舍入到 1.0）下失败形态不同：Python 报错退出，Rust panic | Tauri 命令 panic 体验差；建议 `checked`/`min(h-1)` 后返回 `Err` | 建议修（低） |
| S4 | `prepare_asset.py:17-23` 按**行**写表面单元（同 hash 行重复写 = 后写者胜），relief gain 用该行自己的 `p[2,:2]` | `single_compatible.rs:326-330` 每个 surface 只取**第一个**拥有它的行 | 同一表面 hash、但 Top 区域标志不同的两行：Python 用最后一行的 gain，Rust 用第一行 | 只在这种非典型输入上产生 shader 单元差异（shader 图内容不同 → 立面凹凸/光照观感差） | 建议修（低） |
| S5 | `prepare_asset.py:39` 只改 `p[0,2:]`（`64/roomsize`、`ri*64/roomsize`） | `single_compatible.rs:173-176` 相同 | — | — | 等价 |
| E1 | `asset_transforms.py:35-45` 采样公式：`u=(i+.5)/64`、`yy=floor(mod(u*scale+offset,1)*h)`、`xx=floor(mod((u+state)*scale,1)*w)`、`tile=original.copy()`、非夜景列 `alpha=0`、RGB 不动、`concatenate(axis=1)` | `single_compatible.rs:470-499` | **逐字等价**（含 `state*64+tx` 的 x 排布、`ty` 的 y 排布、`apply_night_only = night_only && scale>0`） | — | 等价 |
| E2 | `prepare_asset.py:16` `roomsize=max(1024,64*2**ceil(log2(len(roomkeys))))`；`36` 行 `maps[3][ri*64:(ri+1)*64,:1024]=strip` | `single_compatible.rs:149` `(64*len.next_power_of_two()).max(1024)`；`450-454` 同区域 | 等价 | — | 等价 |
| E3 | `native_controls.py:22-33 office_room_sheet` + `prepare_asset.py:28-34`（`room.shape==(512,512,4) and p[0,2]==1/16`、源无 alpha 时整表归零） | `single_compatible.rs:420-448`（含 `include_bytes!("../../resources/sc5/office-room.png")`，源文件 sha 相同） | 逐像素等价（32×32 tile、`x=((v%2)*4+state)*32`、`y=(v//2)*32`、`state!=1 → alpha 0`） | — | 等价 |
| S6 | Python 0.8.0 `prepare_asset.py` **没有** sRGB→线性迁移 | `single_compatible.rs:399-419` `migrate_room_rgb`（profile=`native-room-states-transparent-top-v3` 或 `openscp.ny-native/1` 无戳时对 RGB 做 sRGB→线性） | Rust 专有变换，基准里不存在 | 只作用于本仓导出源的室内贴图；与 Packager 用同一输入时会得到不同像素 | 等价（专有扩展，需登记） |

### Q2 六槽材质编译

先说结论：**0.8.0 全链路没有 DXT 压缩**。生产路径只用
`pixFmt=21`（A8R8G8B8 原始 BGRA，`authored_materials.py:27`）与 `pixFmt=116`（Float32 参数纹理，`authored_materials.py:36`）。任务描述里的"DXT 压缩"在基准中不存在，Rust 也不做，二者一致（见 E7）。

| # | Python | Rust | 差异 | 影响 | 级别 |
|---|---|---|---|---|---|
| M2 | `backend_compile.py:122-130` `materialStrategy=='compact_2048'` 时，把 `tint/normal/surface` 中任一边 >2048 的 PNG 按 `ratio=min(2048/h,2048/w)` 逐通道 `BOX` 缩放；`backend_compile.py:43-44` 校验策略枚举 | Rust **完全没有** `materialStrategy`（全仓 grep 无命中）；`single_compatible.rs:316-458 render_atlases` 无任何尺寸上限 | 大图集（surface 唯一组 >16 时 grid≥8 ⇒ 4096²）不会被压到 2048 | 包体膨胀数倍、加载风险；且与作者推荐档位产出的像素不同（2048 档是 GUI 推荐值） | 必须修 |
| S7 | `compile_tower_asset.py:57` `at=32+rw.of_type(0x20004)[0].size` 计算材质纹理表锚点 | `template_material.rs:53-64 find_texture_table` 从 `4+28` 起扫描**第一个对齐的 `0x2D` 字** | 锚点推导方式不同；当前模板上两者一致（已验证产出正确），但换模板/头部含 `0x2D` 时会错位 | 写坏六槽实例号且可能只被后置 decode 校验勉强拦住 | 建议修 |
| S14 | `compile_tower_asset.py:57-59` 只改 `rw.of_type(0x2000b)[0]`（第一个 MATERIAL 段） | `template_material.rs:19` 改**所有** MATERIAL 段的六槽 | 多材质段模板下 Python 只改第一段 | 当前 Building40 模板只有 1 段，实际无差 | 建议修（低） |
| E4 | `prepare_asset.py:14-16` surface 去重键 = `sha256(PNG[4])+sha256(PNG[1])+sha256(PNG[6])`；room 键 = `sha256(PNG[7]) + p[0,0,2] + p[0,0,3]`；`width=2**ceil(log2(count))`、`sg=2**ceil(log2(ceil(sqrt(n+1))))` | `single_compatible.rs:67-102,113-152`（`room_key` 用 `to_bits` 并归一 `-0.0`；`grid` 含 `+1` 兜底格；`width=next_power_of_two`） | 等价（含 `-0.0/+0.0` 的 Python `==` 语义） | — | 等价 |
| E5 | `prepare_asset.py:18-23` 表面单元：`x=(si%sg)*512,y=(si//sg)*512`，`Image.resize((512,512),NEAREST)`，`j==2` 且有 Top 区域时 `a[:,:,0]=clip(round(23+(h-23)/sg))`、`a[:,:,1]=clip(a* sg)` | `single_compatible.rs:331-358`（`round_ties_even`、通道1 截断而非四舍五入、`nearest_resize` 手写 `floor((i+.5)*src/dst)`） | 等价（含 `np.round` 半偶舍入、NumPy `float→uint8` 截断） | — | 等价 |
| E6 | `prepare_asset.py:24-26` 调色板：`s=int(round(p[0,layer]*512))%512`，`dest=i*4+layer*2`，`pal[:,dest:dest+2]=sp[:,[s,s+1]%512]`，`p[0,layer]=dest/512` | `single_compatible.rs:368-387` + `140-143`（`round_ties_even`、`%512`） | 等价 | — | 等价 |
| E7 | `authored_materials.py:29-39 texture_rw4`、`:13-27 raster_rgba` | `author_materials.rs:120-192 encode_float_texture/encode_rgba_texture/texture_wrapper`、`materials.rs:163-223 encode_rgba_raster` | 格式等价：slot0 `fmt116`/`w=2^ceil(log2(count))`,`h=4`/单层/`0x180`；slot1-3 `fmt21` 光栅全 mip 链至 1×1/`0x801c`;slot4 `512×16`；slot5 `roomsize²`；统一 BGRA、大端头 `>6I(2,w,h,mips,8,21)` | — | 等价 |
| E8 | `compile_tower_asset.py:56` `0x2f4e681b if i in (0,4,5) else 0x2f4e681c` | `package.rs:485-489` `materials::rw4_model_type()` for 0/4/5，raster 其余 | 资源类型分配等价 | — | 等价 |
| E9 | `prepare_asset.py:51-52` 兜底格 `normal[-512:,-512:]=[128,128,255,255]`、`shader[-512:,-512:]=[23,255,0,255]` | `single_compatible.rs:360-366` | 等价 | — | 等价 |

逐槽"缺什么"：

- slot0 参数表：**不缺**。布局 `(4,width,4)` little-endian 行主序，`author_materials.rs:99-110` 与 `prepare_asset.py:13,42` 一致。
- slot1 tint / slot2 normal：**只缺 compact_2048 降采样**（M2），内核一致。
- slot3 shader：同上；另 S4（首/末行 relief gain 选择）。
- slot4 palette：**不缺**（公式、目标列、尺寸、fmt 全对）。
- slot5 interior：**不缺**；缺"打包后回读像素 == 中间图集"的校验（S8）。

### Q3 模型几何

| # | Python | Rust | 差异 | 影响 | 级别 |
|---|---|---|---|---|---|
| M3 | `compile_geometry_contract.py:27-31`：`require(len(set(tri))==3)`、`require(cross² > 1e-20,'Zero-area triangle')` ⇒ **拒绝** | `author_building40.rs:324-339`：`if cross_squared<=1e-20 { removed += 1; continue; }` ⇒ **静默删除** | 退化/重复索引三角形：Python 构建失败，Rust 构建成功但少三角形 | 静默几何缺失（外观破洞），且 `backend_compile.py:235-238` 的"三角形不丢"后置校验在 Python 侧会立刻报警、Rust 侧无人报警 | 必须修 |
| M4 | `backend_compile.py:28` `angle=rotation_degrees(settings.get('rotationDegrees',180))`（缺省 180），`prepare_asset.py:44` 对 pos/norm/tan 应用 | `stages.rs:827-829 default_scene_rotation(building40)=0`；`stages.rs:819-825 model_rotation`；`author_building40.rs:195,279-281` 应用 | Building40 缺省旋转：Python 180°，Rust 0° | 若输入是同一种 Blender 导出（正面 -Y，`compile_tower_asset.py:35` 注释 "Front is source -Y"），Rust 会把模型与 path 一起翻到 -Y ⇒ 正面朝反、路径镜像。本仓 `ny_facade/export_asset.py:278` 声明 `sourceFrontAxis:'+Y'`，因此本仓导出的 LOTM 需要 0°——**这是输入坐标约定差异，不是算法差异**，但缺省值不能同时满足两种输入 | 必须修 |
| S9 | `compile_tower_asset.py:7,32` `accessor()` 忽略 `bufferView.byteStride`（按紧密排列读取） | `author_building40.rs:433-437,495-497` 遇 `byteStride` 直接 `Err` | Rust 更严格 | 交错顶点缓冲的第三方 LOTM：Python 会读错（静默错误几何），Rust 拒绝。方向相反，需登记 | 等价（有意收紧）/建议修（记录） |
| E10 | `prepare_asset.py:43-49`：读 `POSITION/NORMAL/TANGENT(4)/TEXCOORD_0/1/3`，`rotate_xyz`，按组内键去重（pos+normal+tan+uvBase+uvTop+seedByte），`tangentSign=1`,`seedByte=clip(round(TEXCOORD_1.y*255))` | `author_building40.rs:220-339`（`VertexKey[14]` 含 seed；`-0.0` 归一；`pack_direction` 用 `127.5+127.5`、w=255） | 逐字等价（旋转矩阵 `+90/270` 是 Python 矩阵的转置表示，结果同为 `(-y,x,z)` / `(y,-x,z)`；矩阵项为 0/±1，f32 计算无舍入） | — | 等价 |
| E11 | `compile_geometry_contract.py:39` UV：`struct.pack_into('<4f', payload, i*40+24, base.x, 1-base.y, top.x, 1-top.y)`；`payload[i*40+22]=materialRow`、`+23=seedByte` | `author_building40.rs:308-313` | 等价（v 只翻一次、行/种子在 22/23 字节、20/21 为 0） | — | 等价 |
| E12 | `compile_geometry_contract.py:33` 模板 profile 校验；`:34` `replace_geometry` 更新 mesh/VA/TA 字数、`:41` 碰撞 | `author_building40.rs:566-699`（`index32 = len>65536`、`mv[3]=tri, mv[6]=flat, mv[7]=min, mv[8]=verts`、`ta[4]=101/102`、BBOX 32 字节 min/max、`replace_box_collision`） | 等价 | — | 等价 |
| E13 | `build_candidate.py:89,112,143`：4 个 LOD 用**同一份**模型字节、不同 instance；`asset[00F9EFBA]`=模型包围盒 | `package.rs:824-840 insert_compiled_model`（4 份同字节、不同实例）；`written_contract.rs:975-990` 用 LOD1 包围盒对 `00F9EFBA` | 等价 | — | 等价 |
| S10 | `compile_geometry_contract.py:8-14` 额外契约校验：`schema/unit/upAxis/uvConvention/materialMode`、`material_rows` 上限 256、每顶点 `materialRow < rows` | `author_building40.rs:181-199` 只校验 LOTM9 / meshes==materials / 1..=128；行号在 `311` 直接写 `group as u8` | 无 uvConvention/materialMode 契约（Rust 结构上跳过该中间 IR，属设计差异） | 无实际影响，但 1:1 意义上少一层显式断言 | 建议修（低） |

### Q4 属性编码

| # | Python | Rust | 差异 | 影响 | 级别 |
|---|---|---|---|---|---|
| S11 | `native_property_codec.py:27-31 validate_native_profile`：每个字段必须 `flags & 0x8000`，否则构建失败 | `property_edit.rs:93-107 encode` → `sc-properties/lib.rs:176-224 encode_canonical`；`canonical_flags:316-347` 对**非零**输入 flags 逐位透传 | 缺"整字典 0x8000"闸门；donor 里若存在 `0x001c` 之类无有效位的记录，Python 拒绝、Rust 原样发出去 | 极端情况下整条属性文件被游戏丢弃（该类事故已有实证） | 建议修 |
| S12 | `build_candidate.py:40-42 prop_bytes`：**未改动字段保留原始 recordHex 字节**，只重编被改字段，并按 hash 排序 | `property_edit.rs:93-107`：**整字典 parse→encode** | Rust 会重写所有记录（值 f32 精确回环，但 String8/空变体/异常 flags 会按 Rust 规则重编） | 对原版 donor 应为字节等同；对含 quirk 的 donor 可能变化 | 建议修（低） |
| E14 | `inspect_park_route.py:26-38 parse`：`>IHH` 头；`flags&0x30` 标量/数组、`&0x40` 空变体；`sizes` 表 | `sc-properties/lib.rs:104-170` | 等价（含 `0x100` 短 String8、Transform `flags 12/13/15`、count 上限） | — | 等价 |
| E15 | `build_candidate.py:31-38` `scalar/array/key/text/vector/floatprop/intprop`：数组 `>IHHII` flags `0x809c`（Key/数值/Transform）、`0x801c`（Text/BoundingBox）、item 12/56/4/520/24；Key 线序 `(instance,type,group)`；Bool 标量 `flags=0x8000` + 1 字节 | `sc-properties/lib.rs:316-347`（`0x809c`/`0x801c` 推导）、`368-466 write_value`（Key 写 instance→type→group、Bool 1 字节、Text table→id） | 等价 | — | 等价 |
| E16 | `prop_contract.py:56` `array(h,typ,payload,count,size)`；`prop_contract.py:77-84` 道具四列（`0x0c12ef20/30/40/50/60 + bin`） | `native_profile.rs:623-739 compile_prop_fields`（Key 数组 / Transform(flags 15, unknown=scale) / Int32 / Bool 标量） | 列形状、类型、flags 等价 | — | 等价 |
| S13 | `prop_contract.py:82` 索引列 = `range(n)`（0,1,2,…） | `native_profile.rs:661-677,712-724` 索引列 = 每个道具对应原型的下标（单原型时全 0，代码注释明确写了这是有意改法） | 有意偏差；Python 的 `0..N-1` 在只有 1 个资源时是越界的 | 静态道具列当前不受 percent 驱动，实机未见差异；但与基准字节不同 | 等价（有意偏差，需登记） |
| E17 | `prop_contract.py:43-49 rotate_matrix`（`c=round(cos)`、`s=round(sin)`，对 4 行含平移同样处理） | `native_profile.rs:576-597 rotate_matrix` | 等价 | — | 等价 |
| E18 | `native_property_codec.py:11-25 encode_lot_numeric`（类型表 + scalar-only + 重新打包 `flags=0x8000`） | `native_profile.rs:394-415 encode_lot_numeric`（类型表 + scalar-only，**不改 flags**，返回 clone） | Rust 不强制 0x8000；原版 donor 本来就有，故实际等价 | — | 等价（低风险） |

### Q5 构建期校验（Python 校验项 → Rust 有无对应）

| # | Python 校验 | Rust 对应 | 结论 |
|---|---|---|---|
| S8 | `backend_compile.py:241-256`：把打包后的 slot5 顶层 mip 解出，**逐像素**等于 `interior.png`；再按 `材质采样变换.json` 逐 state 校验 alpha/`nightUnchanged`/`rgbUnchanged` | `written_contract.rs:1019-1057 validate_slot_payload` 只 decode 顶层 mip，不比像素；无 state 证据 | **缺**（最关键的窗户回归闸门没有对应物） |
| S15 | `verify_package.py:64-70`：材质 Raster 每条 mip 长度必须 `w*h*4` 且链到 1×1；`71-79` 地面 raster 同样校验并禁止残留 albedo | `rw4::RasterImage::parse`（`crates/rw4/src/raster.rs:134-160`）只按 block 长度读，不校验尺寸/末级 1×1 | **缺** |
| S16 | `backend_compile.py:235-238`：`len(model_back.decode()[1]) == 源三角形总数`（"Triangle loss"） | `written_contract.rs:802-820` 只统计，不比对源总数 | **缺**（正好放过 M3） |
| S17 | `verify_package.py:55-57`：碰撞盒 `low/high` == Asset `00F9EFBA`；几何全部落在包围盒内 | `written_contract.rs:975-990` 只对 `00F9EFBA` 与 LOD1 实际包围盒；无碰撞盒回读比对 | 部分缺失 |
| M5 | `verify_package.py:46-49` + `menu_icons.py:6-10`：图标必须 `ImageOps.fit` 归一化到 128×128（LANCZOS），大图必须 454×263 | `menu.rs:2077-2121 build_icon_png` 对 PNG **直通**、不做尺寸归一；`menu.rs:2126-2171` 大图 quality 88 | **缺**：图标尺寸/比例不受控；非 128×128 图标进包后 UI 拉伸/裁切，Python 侧 verify 会直接判失败（必须修） |
| S19 | `verify_package.py:33-45`：locale BOM、自有 Text 必须能在自有表里解析到、`table` 必须指向自有表 | `menu_contract.rs:119-146,316-331` 校验 Text→locale 行可解析 | 等价（覆盖面相当） |
| S20 | `backend_compile.py:55-56,74-81`：输入快照 + 构建中再算 sha 校验（"构建过程中输入文件发生改变"） | Rust `load-inputs` 只读一次，无二次哈希 | **缺**（低，单进程内风险小） |
| S21 | `prop_contract.py:9-29`：道具数 ≤1000、`unit/upAxis/space`、`matrix` 12 项有限、正交性 ≤1e-4、`det≥.999`（拒绝镜像）、位置 ≤1e5、`scale∈[0.001,100]`、资源签名/模型 sha256 必须匹配 `catalog` | `native_profile.rs:645-653` 只校验 `scale∈[0.001,100]` | 缺正交/镜像/数量/资源指纹校验（Rust 输入是 scene.json，来源不同，但同类闸门应补齐） |
| S22 | `light_contract.py:9-33`：≤128 盏、`POINT/SPOT`、矩阵正交+非镜像、颜色 0..1、`radius/length∈[.05,500]`、`innerRadius≤radius`、`intensity∈[0,50]`、`specular∈[0,20]`、`control=='NATIVE_NIGHT'` | `native_profile.rs:761-890` 只做缺省填充（`radius=10/inner=0/intensity=1/...`），**无范围校验** | **缺** |
| S23 | `backend_compile.py:41-44`：`geometryStrategy` 必须是 `stable_single_mesh`，`materialStrategy` 必须是 `stable_baseline/compact_2048` | `geometry_route.rs:58-68` 只允许 `building40`；**无 materialStrategy** | 部分（等于 M2 的另一半） |
| S24 | `backend_compile.py:111-114`：`0<len(chunks)==len(mats)<=128`；每组 `meshes[0].primitives[0]` 必含 6 个属性 | `single_compatible.rs:107-112`；`author_building40.rs:227-240`（且额外校验各属性 count 一致、全部 finite） | 等价（Rust 更严） |
| S25 | `backend_compile.py:51-56`：恰好 1 个 LOTM + 同名 `.lot.package`；`57-63` 游戏目录/3 个基础包存在 | Rust 是 schema 驱动（reference.package + 可省略 lot），语义不同 | 等价（架构差异，非缺口） |

### Q6 构建顺序 / 资源集合

Python 生产资源清单（`build_candidate.py:83-155`，作者公园配方，17 个资源）：

| Python 资源 | 组/TGI | Rust 是否生成 | 备注 |
|---|---|---|---|
| Menu2 | `00B1B104:09878A01:<main>` | ✅ `menu.rs:613-616` | |
| Unit 叶 | `00B1B104:40E0C000:<main>` | ✅ | |
| Asset/Lot 叶 | `00B1B104:40E1C000:<main>` | ✅ | |
| LOD ×4 | `2F4E681B:0:<ids>` | ✅ 4 份同字节 | |
| 地面 mask | `2F4E681C:0:<mask>` | ✅ `native_profile`（`rotate_lot` + raster 重编） | |
| 图标 | `2F7D0004:40E02400:<main>` | ✅ 但**未归一化 128×128**（M5） | |
| 大图 | `3F8662EA:0:<main>` | ✅ 但**不走模型预览**（S26）、quality 88 vs 92 | |
| locale | `0A98EAF0:02FABF01:<table_id>` | ✅ 但 instance 复用身份实例而非独立 table_id（S27） | |
| slot0/4/5 | `2F4E681B:0:<id>` | ✅ | |
| slot1/2/3 | `2F4E681C:0:<id>` | ✅ | |
| **图形单元** | `00B1B104:42E1C000:<main>` | ✅ **Rust 额外生成**，Python 0.8.0 无此资源 | 我方夜景必需（`menu.rs:548-590`），属扩展 |
| 状态映射自引用 | `asset[0CA5CF4A]` 重定向（仅无 profile 分支，`build_candidate.py:114-120`） | ❌ 无对应 | 仅在"无独立 profile"的旧分支出现，生产 Rust 路径不涉及 |

菜单资源内容层补充缺口：

| # | Python | Rust | 差异 | 影响 | 级别 |
|---|---|---|---|---|---|
| S26 | `build_candidate.py:148-155`：菜单大图 = 项目预览 PNG `ImageOps.fit(454,263)`，JPEG quality 92 | `menu.rs:2126-2171 build_marquee_jpg`：仅 `marqueePng` 项目 JPG 或按实例散列生成渐变占位，quality 88，**从不使用模型预览** | 图源与质量都不同 | 菜单 rollover 大图是占位渐变，不是资产外观 | 建议修 |
| S27 | `build_candidate.py:103,105,145-147`：locale 资源独立 `table_id`，JSON `indent=4`、`"0x00000001": "…"`、末尾含 `"0xffffffff":"0xffffffff"` | `menu.rs:1672-1693 build_locale`：instance 复用身份实例；`{\r\n    "0x00000001" : …\r\n}`（冒号前有空格），无 `0xffffffff` 行 | 资源身份 + JSON 字节形状不同（语义键 1/2 相同） | 不改变显示文本；与基准字节不同、空描述时少一行 | 建议修（低） |

构建顺序差异（不影响外观，影响可诊断性）：

- Python：`settings 校验 → inspect_inputs → 快照 → LOTM 头/组/属性检查 → prepare(几何+材质+房间) → compact_2048 → material_factory → build_candidate(菜单/地块/打包) → verify_package → 回读三角形/内景/朝向 → 复制到 output`（`backend_compile.py:23-281`）。
- Rust：`load-inputs → insert-textures → insert-model(+G1 材质指纹闸) → validate-paths → compute-identity → property-patches → scene-semantics → behavior-bind → layout-contract → menu-contract → delta-audit → vanilla-shadow-strip → material-binding-contract → identity-closure → golden-shape → materialize-and-link → written-contract`（`stages.rs:128-143`）。
- 缺的两步在 Python 的"回读后处理"：内景逐像素比对（S8）与三角形总数比对（S16）；Rust 用 `delta-audit`/`property-gate`/`material-gate` 换成了**回归基线对拍**（比 Python 更强的回归保护，但不覆盖"本次构建是否正确"）。

---

## 3. 必须修清单（可执行修改建议）

编号与第 2 节表格一一对应。**修复顺序**（按外观影响从大到小）：
`M4 朝向` → `M3 退化三角形` → `M1 夜景策略` → `M2 图集降采样` → `M5 图标归一化`。

### M1 `nightEmissionPolicy` 未接入构建
- 位置：`src-tauri/src/asset_build_m1/package.rs:482` `compile_materials(&doc, true)`；`package.rs:454-461 insert_scene_model` 签名无策略参数。
- 修改：
  1. 在 `IntermediateModel.scene`（`asset_build_m0` schema）增加 `nightEmissionPolicy: Option<String>`，枚举仅 `night-only|preserve-source`，缺省 `night-only`（与 `backend_compile.py:29` 一致）；非法值 fail-closed。
  2. `insert_scene_model(..., night_only: bool)` 透传到 `compile_materials(&doc, night_only)`。
  3. 阶段事件记录 `nightEmissionPolicy`，并在 `reports/stage-*.json` 落盘，便于对拍 Python 的 `native-backend-result.json`。
- 验收：同一 LOTM 用 `preserve-source` 构建，slot5 白天列 alpha 非零（与 Python 输出一致）。

### M2 `compact_2048` 降采样
- 位置：`single_compatible.rs:316-458 render_atlases` 之后新增一步（或 `author_materials.rs:74-96` 内、`encode_raster` 前）。
- 修改：当 `plan.atlas > 2048` 且策略为 `compact_2048` 时，对 tint/normal/shader 三张图做逐通道 `BOX` 重采样到 `ratio=min(2048/w,2048/h)`（复刻 `backend_compile.py:125-130`，注意 **逐通道独立**、不得用带 alpha 预乘的库函数）。策略缺省建议与 GUI 推荐一致（`compact_2048`），并在 `written_contract` 记录最终图集尺寸。
- 验收：17+ 唯一表面组的模型，slot1-3 顶层尺寸 ≤2048，UV 区域参数不需改变（参数是归一化的）。

### M3 退化三角形必须失败而非静默删除
- 位置：`author_building40.rs:334-337`（`if cross_squared(points) <= 1e-20 { removed += 1; continue; }`）。
- 修改：改为 `return Err(format!("group {group}: degenerate/zero-area triangle #{index}"))`，与 `compile_geometry_contract.py:27-31` 完全对齐；`AuthorModel.removed_degenerate_triangles` 保留字段但恒 0（或删除）。
- 同时在 `written_contract.rs` 增加"三角形数 == 源三角形数"（源数可从 `AuthorModel.source_triangles` 传到 `WrittenContractSpec`），补 S16。
- 验收：构造一个含重复索引三角形的 LOTM，Python 与 Rust 都拒绝。

### M4 默认旋转必须按输入坐标约定决定（禁止无条件 0°）
- 位置：`stages.rs:827-829 default_scene_rotation`、`stages.rs:819-825`。
- 修改：读 LOTM `diagnostics.sourceFrontAxis`（本仓导出器写 `'+Y'`，见 `src-tauri/resources/blender/ny_facade/export_asset.py:278`）：
  - `'+Y'` → 0°（保持现状，s31 GOLDEN 不变）；
  - `'-Y'` 或缺失 → 180°（等同 `backend_compile.py:28` 的缺省）。
  显式 `semantics.menu.rotationDegrees` 仍最高优先。
- 验收：用 Packager/SC5 插件导出的 `tall_test.lotm`（正面 -Y）构建，正面朝 +Y；用本仓导出构建，字节不变（golden 对拍）。

### M5 图标必须归一化到 128×128
- 位置：`menu.rs:2077-2121 build_icon_png`。
- 修改：无论输入是 PNG 还是其它可解码格式，统一 `load → to_rgba8 → ImageOps.fit 语义 (128,128, Lanczos)`；无法解码或尺寸不符→`Err`。同时补 `written_contract` 的图标尺寸断言（对应 `verify_package.py:46-49`）。
- 备注：当前"PNG 直通"会把 240×160 之类的图原样打进包，游戏 UI 会拉伸/裁切，且 Python 的 verify 会直接判失败。

---

## 4. 建议修清单（按性价比排序）

1. **S8 内景回读像素比对**：在 `materialize-and-link` 之后、发布之前，把 slot5 资源 decode 成 RGBA 与 `SurfaceAtlases.interior` 逐像素比较（Rust 侧同一进程内本来就有两者，成本极低）。这是唯一能拦住"窗外观感错位/窗格全黑"的像素级闸门。
2. **S16 三角形总数比对**：见 M3 的配套项。
3. **S11 `0x8000` 闸门**：在 `property_edit::encode` 前遍历 `file.values`，`encoding.flags & 0x8000 == 0` ⇒ `Err(format!("Property {hash}: outside verified native profile"))`；与 `native_property_codec.py:27-31` 同文案级别。
4. **S15 材质 Raster mip 链校验**：`written_contract::validate_slot_payload` 对 `RASTER_TYPE` 增加 `mip.len()==w*h*4` 且末级 1×1（复刻 `verify_package.py:64-70`）。
5. **S17 碰撞盒回读**：`written_contract` 解 `0x80003`，断言 `low/high` 与 `00F9EFBA` 一致（容差 1e-5）。
6. **S21/S22 道具与灯光数值闸门**：`compile_prop_fields` 增加矩阵正交/`det≥.999`/数量上限；`compile_light_fields` 增加 `radius/intensity/specular/length` 范围与 `innerRadius≤radius`；错误文案沿用 `prop_contract.py:16-28` / `light_contract.py:28-31`。
7. **S7 纹理表锚点**：`template_material::find_texture_table` 改为 `anchor = 32 + vertex_format_section.size`，并保留 `0x2D` 扫描作为断言（两者不等则报错），与 `compile_tower_asset.py:57` 同源。
8. **S26 菜单大图**：`build_marquee_jpg` 在无 `marqueePng` 时改为"用项目预览 PNG（若存在）`ImageOps.fit(454,263)`，JPEG quality 92"，否则才用占位渐变；与 `build_candidate.py:148-155` 对齐。
9. **S27 locale 形状**：`build_locale` 改为 `indent=4` + `"0x00000001": "…"`（冒号后无空格）+ 补 `"0xffffffff": "0xffffffff"` 行；与 `build_candidate.py:145-147` 字节级对齐。locale 资源 instance 改为独立分配（Python `table_id`），避免与身份实例混用。
10. **S1 room-state 证据**：`render_room_strip` 返回 `Vec<RoomStateEvidence>`，在 `reports/` 落盘（字段名沿用 `alphaBefore/alphaAfter/rgbUnchanged/nightUnchanged`），供后续与 Python `材质采样变换.json` 对拍。
11. **S20 输入不可变**：`load-inputs` 记录每个输入的 sha256 到 ctx，`materialize-and-link` 前重算一次；不一致即失败（`backend_compile.py:74-81`）。
12. **S3 边界钳制**：`render_room_strip` 的 `xx/yy` 用 `min(dim-1)` 钳制后返回 `Err` 说明来源非法，避免 panic。
13. **S4 表面单元写者语义**：改为"后写者胜"（遍历所有行覆盖写），与 `prepare_asset.py:17-23` 一致；或对"同 surface 不同 topEnabled"直接 `Err`（更安全，但属收紧）。
14. **S14 六槽改写范围**：`rewrite_texture_instances` 只改第一个 MATERIAL 段，或明确断言模板只有一个 MATERIAL 段（当前事实）。
15. **S12 属性字节保真**：`property_edit` 引入"未改动的记录沿用原 recordHex"路径（`cloned + 定点覆盖`），与 `build_candidate.py:40-42` 一致；可先只在 `encode` 前做等值自检（parse→encode→parse 语义相等）。
16. **S10 几何契约断言**：在 `compile_model_with_transform` 入口补 `unit/upAxis/uvConvention` 之类的显式常量断言（Rust 没有中间 IR，只需一条 debug 断言/注释锚点即可），使"1:1 少一层断言"这条差异可关闭。

---

## 5. 确认等价清单（后续不要重复排查）

> 本清单是合并版（16 条），与第 2 节 E1–E18 的编号不是一一对应（个别条目被合并、少数新增判据）。

1. **room_strip 采样与 16 态布局**：`asset_transforms.py:35-49` ≡ `single_compatible.rs:470-499`（含 `u=(i+.5)/64`、`mod` 语义、`concatenate(axis=1)` 的 x 排布、非夜景 alpha 清零、RGB 不动）。
2. **room atlas 尺寸/条带位置**：`prepare_asset.py:16,36` ≡ `single_compatible.rs:149,450-454`。
3. **Office 房间表修正**：`native_controls.py:22-33` + `prepare_asset.py:28-34` ≡ `single_compatible.rs:420-448`；`office-room.png` sha256 相同。
4. **surface/room 去重键与 power-of-two 网格**（含 `+1` 兜底格、`-0.0` 归一）：`prepare_asset.py:14-16` ≡ `single_compatible.rs:67-102,147`。
5. **表面图集填充与 relief gain 公式**（`round` 半偶、通道1 截断、NEAREST 采样）：`prepare_asset.py:18-23` ≡ `single_compatible.rs:331-358`。
6. **调色板采样/目标列/重写顺序**：`prepare_asset.py:24-26` ≡ `single_compatible.rs:368-387`。
7. **参数表四行重写顺序与数值**（`/sg`、cell 原点、`p[0,2:]`、`p[3,2:]*=sg`、`p[2]` 兜底）：`prepare_asset.py:37-42` ≡ `single_compatible.rs:157-185`。唯一差异是 f64→f32 双舍入在极端值上可能 1 ULP（参数纹理，外观不可见）。
8. **六个 payload 的编码形态**：fmt 21/116、单层 vs 全 mip 链、BGRA、大端 `>6I`、`0x180/0x120` 标志、尺寸（`w=2^ceil(log2(n))`/`512²`/`512×16`/`roomsize²`）：`authored_materials.py:13-39` + `compile_tower_asset.py:53-55` ≡ `author_materials.rs:120-192` + `materials.rs:163-223`。
9. **六槽 TGI 类型分配**（0/4/5→`2F4E681B`，1/2/3→`2F4E681C`，group=0）：`compile_tower_asset.py:56` ≡ `package.rs:485-489`。
10. **模板/碰撞源二进制**：5 个文件 sha256 与 `engine/` 完全一致。
11. **几何核心**：旋转矩阵语义、组内去重键（含 seed、UV 未翻转）、`1-v` 只翻一次、法线/切线打包 `127.5+127.5`/w=255、`D3DCOLOR=(0,0,row,seed)`、索引全局偏移、`index32 = len>65536`、BBOX 32 字节、碰撞盒缩放 + KD 树 split 缩放 + `validate_tree`：`prepare_asset.py:43-50` + `compile_geometry_contract.py:15-45` + `collision_proxy.py:56-91` ≡ `author_building40.rs:204-368,566-699` + `collision.rs:332-424`。
12. **属性线格式**：`>IHH` 头、数组 `>IHHII`、flags `0x809c`/`0x801c`、Key 三元组 `(instance,type,group)`、Bool 标量 `0x8000`+1B、Text `(table,id)`、按 hash 排序：`inspect_park_route.py:26-38` + `build_candidate.py:31-38` ≡ `sc-properties/lib.rs:104-170,176-224,316-347,368-466`。
13. **Lot 数值类型表与 scalar-only 校验**：`native_property_codec.py:11-25` ≡ `native_profile.rs:132-150,394-415`。
14. **地块旋转**：`asset_transforms.rotate_lot`（size/period 交换、offset 旋转、`np.rot90` CCW）：`asset_transforms.py:20-26` ≡ `native_profile.rs:419-458`。
15. **道具/灯光矩阵量化旋转**：`prop_contract.rotate_matrix` ≡ `native_profile.rs:576-597`（对 4 行含平移）。
16. **地面字段**：`0x0FE01484`（`off±size/2, z=max`）、`0x0BBBCDED/2A`、`0x0CCB7FC8/C9/D0/D6`、原生地面图集 `A0DA9E6C`/`60E7805D` 必存在：`backend_compile.py:158-173` + `build_candidate.py:121-125,200` ≡ `native_profile.rs:109-129,1490-1517`。

---

## 6. 判据与证据（复核方式）

- Python 侧全部为逐文件通读（`engine/` 17 个 `.py` 合计 <1000 行 + 根目录 `asset_transforms.py`/`prop_contract.py`/`backend_compile.py`/`audit_window_night.py`/`menu_icons.py`/`light_contract.py`）。
- Rust 侧定位方式：`grep -n "^pub fn / fn "` 列函数表 → Read 对应函数体（`single_compatible.rs` 1-507、`author_materials.rs` 全文、`author_building40.rs` 156-705、`package.rs` 380-760、`stages.rs` 815-940、`native_profile.rs` 369-890、`property_edit.rs` 全文、`crates/sc-properties/src/lib.rs` 全文、`collision.rs` 332-500、`written_contract.rs` 700-1060、`template_material.rs` 全文、`menu.rs` 520-700/1672-1693/2077-2171）。
- "影响渲染外观"的判定只看：顶点/UV/索引、六槽像素、slot5 内景 strip、法线切线、包围盒/碰撞、菜单图标与大图、朝向。
- 已刻意**不做**的事：未运行 Python 打包、未运行 `cargo build/test`、未写入除本文件外的任何文件。
