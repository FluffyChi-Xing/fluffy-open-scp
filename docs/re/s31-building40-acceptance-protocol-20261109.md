# s31 × Building40 契约回归验收协议（2026-11-09）

状态：生产验收清单。金标准 = s31 大学（行为/菜单/窗户内景）的**契约等价**，不是与 Python Packager 包逐字节相等。

## 前置

- 构建入口：工作台「构建项目」→ `build_queue_enqueue_project`（禁止生产路径粘贴 schema / 外部 `.pyc` Packager）。
- 几何：`scene.modelShape` 缺省或显式 = `building40`。
- 属性：menu/behavior/path/props 走 s31 GOLDEN 配方（`menu.rs` 三对 text↔param、`behavior` bundle 链）。

## 固定顺序（死命令）

```text
1. 构建 ledger 绿
2. MANIFEST + SHA-256 登记
3. 清旧测试包 → 部署到 disposable city
4. 冷启动 menu-load oracle
5. 放置/详情/窗户对照 s31
```

### 1. 构建 ledger

在 Actions 页或 `.build/ledger/<build_hash>.json` 确认：

| 阶段 / 字段 | 期望 |
|---|---|
| `insert-model.geometryRoute.name` | `building40` |
| `insert-model.building40Contract` | 六槽实例非 0，`slot5Interior` 有值 |
| `material-binding-contract` | 无空 raster 违规 |
| `menu-contract` | 通过；无 park-style 2-row 统计 |
| Lot `0F169C46`（night interior） | true（scene/package 写回） |
| `delta-audit` | 仅声明族漂移 |

失败时只重跑责任阶段测试，禁止整包人工字节对拍。

### 2. 归档台账

```powershell
python scripts/archive_test_package.py archive <package> --note "Building40 s31-contract <reason>"
python scripts/archive_test_package.py status
```

对部署包与构建产物分别算 SHA-256；**参照必先验哈希**。known-working 标 `GOLDEN-known-working`，永不覆盖。

### 3. 部署隔离

- **禁止**在含健康测试建筑的存档城市换 `000_*.package`。
- 只用一次性城市（新建/另存为）。
- 会话前备份 `Documents/SimCity/Games/`。
- 若数据正确但画面仍坏：隔离 `SimCityUserData/GraphicsCache.package`（改名 `.bak`）后重试。

### 4. Menu-load oracle

冷启动主菜单：

- **瞬间出现** → 继续
- **长时间挂起** → 包坏了；**不要进城市、不要放置、不要存档**；归档为 broken 并回滚包集

### 5. s31 契约对照（一次性城市）

| 检查 | 通过条件 |
|---|---|
| 菜单身份 | 新命名条目，非同 TGI 覆盖参照大学 |
| 详情面板 | 无 `~population_amount~` / `~maintenance_cost~` / `Title` / `Left1`… |
| 夜间窗户 | 内景亮灯，非立面黑块 |
| 放置 / 铲除 | 可放置、可铲除 |

## 诊断分流

- 占位符原样显示 → Menu2 统计行 text↔param 或 Unit 绑定失败（不是 locale 编码）
- 立面黑窗 → 六槽 / slot5 / `0F169C46` / room-state 源 / 构建缓存陈旧
- 菜单挂起 → 包结构坏；立刻卸包

## 相关代码

- `src-tauri/src/asset_build_m1/{author_building40,author_materials,menu,menu_contract,stages,workbench_logic}.rs`
- `docs/re/lotm-to-package-migration-20261108.md`
- `AGENTS.md`（参照哈希 / MANIFEST / 存档隔离 / menu-load oracle）
