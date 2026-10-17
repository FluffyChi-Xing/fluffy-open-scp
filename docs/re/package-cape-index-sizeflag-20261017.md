# cape 包拒绝打开：索引可选字段标记缺失

日期：2026-10-17。已定位并通过同次游戏启动中的原包/修正副本对照确认；Rust 和 TypeScript writer 已修复。现有游戏目录中的两份原包未修改。

## 根因

DBPF 索引的 stored-size 字段最高位 `0x80000000` 表示条目尾部存在额外的 compression-type/commit 四字节，**不表示资源一定被压缩**。最高位为 0 时，游戏只读到 decompressed-size；最高位为 1 才继续读取这四个字节。未压缩资源也可以使用带额外字段的形式，compression-type 写 0。

项目两个 writer 都固定输出 28 字节条目，却把 stored-size 写成无最高位的裸长度。游戏读完首条后只前进 24 字节，将其尾部的四个零字节当作第二条的 type，后续 TGI/offset/size 全部错位。这里普通索引前导共享 unknown，未共享 type/group。

单 TGI 包没有下一条会错位，所以之前单条包能打开并命中的结论仍成立，但不能外推为 writer 的多条资源协议正确。原先 writer→项目 parser 的 roundtrip 和 Rust→TS fixture 对拍共享了同一错误，未能检测游戏兼容性。

项目 `index.rs` 当前无条件读取每条的 compression/flags，因而能解析旧 writer 的错误格式。此次保留旧导出文件的可读性，没有把现有 parser 宣称为严格游戏格式校验器；合法的无额外字段索引形式、旧错误文件兼容、严格检查须在后续 parser/profile 工作中分开处理。

## 静态与运行证据

EXE SHA-256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。

- 实际 index 对象 vtable 为 `0xD3CFB8`；vslot+0x34 的 ReadFromMemory 为 `0x945710`，vslot+0x40 的 Validate 为 `0x945020`。
- ReadFromMemory 的机器码按 size 字段符号位决定是否额外推进四字节；dev 同名函数也保留该逻辑。IDA 伪 C 有栈恢复警告，因此以原始指令和实机对照为准。
- DatabasePackedFile::VerifyIndexRecordIntegrity 为 `0x8E92E0`（vslot+0x80），不是 CreateIndex `0x8E8610`（vslot+0x84）。
- Header 校验 `0x8E9080` 两种包都通过；ReadIndexRecord `0x8EA8B0` 在 VerifyIndex 失败后返回 false，最终 Packed Open `0x8E8C70` 返回 false。

确定性证据见 [JSON 事件与机器码摘录](evidence/cape-index-sizeflag-20261017.json)。有效对照运行 PID 54448，30 秒，6 个入口在恢复进程前验证字节并安装观察，48 条筛选事件，errors=0、dropped=0。

| 同次运行中的文件 | VerifyHeader | VerifyIndex | ReadIndex | Open |
|---|---|---|---|---|
| Data/000_openscp_cape_sizeflag_probe.package | true | true | true | true |
| Data/cape-trinity-clearwater.package | true | false | false | false |
| Data/Oppie_TrainStation.package（已有对照） | true | true | true | true |
| UserData/Packages/cape-trinity-clearwater.package | true | false | false | false |

修正仅改变以下四个 stored-size 的最高位；实际仅四个字节由 00→80，文件长度 3,418,988、索引长度 120、payload 全部保持不变：

| 字段偏移 | 原长度 | 修正字段值 |
|---|---:|---|
| 0x78 | 156 | 0x8000009C |
| 0x94 | 35,398 | 0x80008A46 |
| 0xB0 | 623 | 0x8000026F |
| 0xCC | 3,382,595 | 0x80339D43 |

原包 SHA-256：`de7b3aea44b537f35fff3060f7bb83d88a8f370522ce2a3ed2de3a06440de450`。
修正副本 SHA-256：`a5be968cda592ee69e4e7428f3d03ff1fa6be695bf528fadc63f6cb2b81c3ca3`。
修正副本保存在本地 `tmp/package-loader-20261017/cape-sizeflag.package`，未覆盖原包。

探索日志范围：最初 `package-cape-failure-20261017` 将 CreateIndex 错标为 VerifyIndex，此条事件不用于结论；随后纠正到 vslot+0x80。`package-cape-sizeflag-20261017` 在旧实例未退出时提前终止、没有有效对照，亦不用于结论。上述表格全部来自纠正后的 `package-cape-sizeflag-confirmed-20261017`。

## 修复及验证

- Rust `crates/dbpf/src/writer.rs` 写 `len | 0x80000000`，仍保留未压缩 type=0；拒绝超出 31-bit size 的资源。
- TypeScript `src/lib/game-ui/dbpf-writer.ts` 同步设置最高位；更新 Rust 生成的共享 fixture。
- 两侧增加按游戏可选字段规则推进游标的多条资源测试，不调用项目 parser 来充当兼容性裁判。Rust 还包含空 payload。
- `cargo test -p dbpf --test writer`：4 通过；`npx vitest run src/lib/game-ui/binary-writer.test.ts`：8 通过；`git diff --check` 通过。

这次实机观察止于容器 Open。没有声称地图语义、注册后的覆盖选择、区域生成或城市加载均通过。旧导出包不会随代码修复而自动更新；应重新导出，或对经诊断确认同一格式的文件制作修正副本，不能对所有 package 盲目按固定偏移打补丁。

收尾已确认游戏退出；投放的测试包经哈希核验后删除，两个原包哈希保持不变。未打开或保存城市。
