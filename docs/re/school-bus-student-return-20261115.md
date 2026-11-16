# 校车放学返家与学生载荷去向

截图只提到放学返家时校车吞学生。市内接送与跨城接送分别取证。两条线均来自本机零售脚本包的 AEB `08068AEB:40800200:622B9CD7`，包与 AEB 哈希见[教育规则取证](educator-rules-copy-paste-20261115.md#查证范围)。

## 市内返家

- 小学放学的 `4E02A030 / 157CF4E7 / C80DE492` 从学校扣 `Student`，加回席位，转出 `KidE2` 和 `KnowledgeToken`。第一条还传递跨城标记 `FromTradingConnection`。
- 校车链中的 `EC1FC9E7` 将本地 `KidE2` 转为事务侧的 `KidE2`，增加 `SchoolBusPassenger`。`C5B7BDCB` 提供反向载荷转移，扣掉乘客标记并把 `KidE2` 落入站点。
- 通向住宅的 `7BFDBB71 → 64BDAB87` 选择 sink `5770FEDD`。前者写入学生和知识载荷，后者仍传递学生，但该规则自身不输出知识。住宅链的 `414C4DED` 消费事务里的 `KnowledgeToken`，`26CCFB8F` 检查 `KidE2` 再写 `StudentTokenE2`。需要观测选择的是哪条分支，单看返家人数不足以判断知识是否回传。

## 区域校车的乘客回收

发行版 `40E0C700:FF3BCAAD` 的 sink 对照表将 `2A7BE5FA` 绑定到 `724063DA`。该规则恢复本地 `SchoolBusR2` 车辆资源，并进入 `B7D543C7 → 646B4105 → EFC1EE83 → 2E2CB131 → B853E58B`。

链中的五条规则各自有条件地消耗事务侧 `KidE2` 和 `SchoolBusPassenger`。前两条写全局 `CB6ED638`，后三条写全局 `B679F814`。五条都增加 `SchoolBusTripsToday`，都没有把 `KidE2` 或 `KnowledgeToken` 转交住宅。两版 5/2/14 零售库都能定位这段链。287520926 的 `724063DA`、`646B4105`、`B853E58B` 记录偏移分别是 `0x247565`、`0x247685`、`0x247835`。

`B679F814` 有后续消费者，例如 `127AD466` 消耗一份全局资源并新增一个本地 `Kid`。另一池 `CB6ED638` 见 `59F22A20`，它与 `ImportedKid` 一起消耗。这里能证实的是乘客载荷转为全局资源，原住宅与本次知识载荷没有在这些操作中传递。全局资源是否随后兑现、是否补到原城，以及截图所指问题到底发生在哪一段，仍待实机核验。

另有显式的跨城学生送回站点路径。`AF13625E` 和 `BFF06FEC` 将 `KidE2`、`KnowledgeToken` 与 `FromTradingConnection` 发送到 `13204670`。其配对资源 `40E0C700:DCDAB86A` 绑定接收规则 `D74CCCAB`，接收 `KidE2` 并把知识量保存在本地 `8B91E44E`。区域校车回收链是否抢先处理本应走这个入口的学生，是可证伪的假设。

## 复现与修补判据

先测同城，再测相邻城市各一所学校和一个出发住宅区。在首个放学周期按同一事务追踪车上 `KidE2 / SchoolBusPassenger / KnowledgeToken / FromTradingConnection`、到达 sink、规则 ID、两个全局池和住宅的 `Kid / StudentTokenE2`。若 `724063DA` 的乘客清理链执行且住宅侧缺少相应交付，进一步检查全局池是否兑现。若同城也会丢学生，应独立追查 `5770FEDD` 交付失败与事务销毁。修补时应同时检查重复下车、学生数守恒和知识回传，避免增加第二次交付。

本次只读解包，未运行模拟或修改安装。

## 邻城考试、住宅教育和工业科技

零售 287520926 的三条接线分别使用不同资源，不能用考试数代替住宅教育或工业科技验收。

| 指标 | 原版路径 |
| --- | --- |
| 邻城考试 | 区域校车链 `6FD24505` 和 `5C91463F` 写 `ExamsPassedE2AtNeighbor`。`8911D948` 清理该指标。对主 AEB 的全部 UnitRule 资源操作扫描中，该全局资源只出现在这三条规则。 |
| 住宅教育 | 住宅返家 sink `5770FEDD` 的后续链 `414C4DED → 26CCFB8F` 消费事务的 `KnowledgeToken`，并在存在 `KidE2` 时增加本地 `Student` 与 `StudentTokenE2`。回收入口仅清车并写全局池时，这条住宅交付没有随该操作发生。 |
| 工业科技 | 教育设施的 `487CDA43` 与 `487CDA42` 注册 E2/E3 技能派发。它们经 `C06BA882/83` 检查 `TotalStudents`，在教学时段产生本地 `SkillTokenE2/E3`，由 `1D9313F0/F1` 等经 Pipe handler 输送。接收端 `3565A387/86` 把在途技能写进接收建筑，后续规则设置 `SkillLevelE2/E3` 并发布地图层 `AFC08B7C/D`，工业选址规则 `80861E4C/D` 读取这些层。区域校车链没有对应的技能载荷。 |

EA 手册分别列出 Education 和 Industrial Tech 两张地图。MaxisGuillaume 对工业科技可视化的说明也把紫线描述为从学院和大学到工厂及特定电厂的 Pipe agent。现有脚本证据支持将「原城住宅没有收到知识」作为校车修补的验收点。给原城工业补发科技需要独立机制及验收。

外部资料：[EA 游戏手册](https://akamai.cdn.ea.com/eadownloads/u/f/manuals/GAME-SIMCITY/SimCity_2013.pdf) · [MaxisGuillaume 的工业科技可视化说明](https://community.simtropolis.com/files/file/29421-the-purple-line-in-industrial-tech-map-mod/) · [EA 论坛玩家报告](https://forums.ea.com/discussions/simcity-franchise-discussion-en/re-education-bug-enrollment-drops-to-55/10356556)
