# lotm→游戏文件唯一路线：SC5 Asset Packager → Rust Building40（2026-11-08，修订 2026-11-09）

状态：**生产路径 = Rust `asset_build_m1` Building40**。裁决人：用户。

## 裁决链

1. 2026-11-08 死命令：lotm→游戏文件完整迁移 SC5 Asset Packager 唯一路线，弃用我方早期模型合成路线
   （先例：blender→lotm 半个月卡死，完整迁移 SC5 Asset Editor 一步解决）。
2. 2026-11-08 范围修正：**迁移只解决模型**（几何/朝向/材质/RW4/地面栅格/打包）；
   **道具、灯、行为属性以我方 s31 GOLDEN 配方为准**（树/path/垃圾桶/车 bin/light 已实机验证）。
3. **2026-11-09 生产路径修订（死命令更新）**：Packager 算法已迁入 Rust
   （`author_building40` / `author_materials` / `single_compatible`）。**正式构建禁止再
   spawn `.pyc` / Packager exe / `backend.command`。** Python Packager 仅作离线算法
   对照 oracle，不是生产入口。工作台只做场景编排 + 逻辑编排；建筑编译由构建器
   Building40 内核完成。

## 当前架构

```
工作台 Scene / Logic
  scene document + semantics.menu/behavior + dependencies
        │
        ▼
build_queue_enqueue_project → workbench_logic::compile → openscp.asset
        │
        ▼
asset_build_m1 stages
  load-inputs → insert-textures → insert-model(Building40)
  → scene-semantics → behavior-bind → menu-contract
  → delta-audit → materialize-and-link
```

| 层 | 职责 |
|---|---|
| 工作台 | path/props/地块、菜单绑定、行为三原语 |
| Rust Building 核 | LOTM→单 mesh Building40、六槽、朝向、`0F169C46` |
| Rust 编排核 | scene delta、menu 新身份、behavior、delta-audit |

`scene.modelShape`：**缺省或显式只能是 `building40`**。`nh-atlas` / `merged-single`
不再是生产入口（schema fail-closed）。

## 历史 E2E 证据（Packager 对照，非生产）

- 输入：`tall_test.lotm`（115,158 三角形 / 112 材质组）+ 游戏提取地块
- 当时命令：`runtime/sc5_packager.pyc build projects/tall_test`
- 产出 sha256 `0098d572433f505cdd688a11dc06f50bfb67e2e362836b3dcf0203efb677e8e2`
- 用途：证明 Packager 模型路线可行；正式资产必须走 Rust + s31 属性配方

## 验收金标准

s31 大学的**契约等价**（不是与 Python 包逐字节相等）：

1. 构建 ledger：menu-contract / night-interior / delta-audit 绿
2. 部署前 MANIFEST + SHA-256 登记；一次性城市
3. 冷启动 menu-load oracle
4. 窗户内景、详情无 `~population_amount~` / `Title` / `Left1`

详见 `docs/re/s31-building40-acceptance-protocol-20261109.md`。

## 非目标

- 不把 Packager GUI / preview / prop 库迁进 Rust
- 不恢复 mesh-splitting 几何路线
- 不把场景 path/props 交给 Packager 公园 legacy 字段
