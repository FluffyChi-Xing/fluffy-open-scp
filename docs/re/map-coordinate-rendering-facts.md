# 地图坐标、UnitTransform 与渲染故障账本

## 坐标边界

游戏数据使用右手 Z-up 坐标；OpenSCP 地图场景使用 map-local `(x, height, y)`，最终由 `map-world` 根节点翻转 Z：

```text
game (x, y, z) → map-local (x, z, y) → scene (x, z, -y)
```

实现入口：`src/components/map/map-coordinates.ts`。

所有图层必须只做一次边界转换。高度、资源 brush、道路曲线和 Unit 模型不能各自重复交换 Y/Z。

## UnitTransform

区域 EGB 的 Unit 记录保存：

```text
id, scale, rotation[9], position[3]
```

道路交叉口模型通过 `00F9EFBB` 找到 RW4 模型，再使用 UnitTransform 放回区域世界坐标。当前代码将 `cSPTransform` 的三个轴向量按连续存储解释，生成 map-local 矩阵，并单独处理索引 winding。

CapeTrinity 两个 `30360939` 实例的数值样本和回归测试在 `map-native-roads.test.ts` 中固定。道路网络已由用户确认连通，因此后续变换修改必须先以这两个实例和道路端点为回归基线。

## 已修复的预览问题

- 只使用 `site == "0"` 的区域道路曲线，避免把城市已建道路当成区域原始网络；
- 曲线按 PathEntry 分组，并使用端点容差切成连续链；
- 不再用 ED 栅格猜测真实道路材质和车道线；
- PathEntry 的静态模型、ribbon、纹理和 junction model 分开处理；
- 地图图层控制器支持道路、植被、水面和资源图层的开关。

## 当前对拍中确认的问题

### 挡土墙/隧道变成水平面

用户对拍显示道路中心线正常，但桥梁两侧挡土墙和隧道附件会出现大片水平混凝土平面。这说明问题位于：

- RW4 模型局部轴的解释；
- sweep 组件局部旋转/缩放；
- component model fallback 选择；
- 或模型本身的静态/实时 extrusion 版本选择。

它不是道路拓扑断裂，也不能通过修改曲线连接来解决。必须按具体模型 ID、PathEntry 行号、模型 bounds、局部旋转和材质逐项对拍。

### 交叉口模型错位

曾经尝试把旋转数组当作行主序矩阵，导致用户对拍出现交叉口与道路段错位。当前基线已恢复 `cSPTransform` 轴向量连续存储的解释；后续如果再次调整，必须同时检查：

1. 交叉口模型边界；
2. 30360939 静态点变换后的道路端点；
3. EGB 曲线端点；
4. map-world 根节点的 Z 反射。

## 未解决问题

1. 挡土墙和隧道具体 RW4 模型的正确 sweep 轴、端点锚定和局部旋转尚未实锤。
2. 模型 normals、材质 pass、透明度和双面状态还没有与游戏逐项对拍。
3. 路口模型与运行时曲线的 connector 选择规则尚未完全恢复。
4. 无 EGB 时的区域 UnitTransform 和曲线切分还不能自动重建。

## 回滚与验证

道路基线提交：`dea5f677f22558f887c5138a3b2679073538077a`。

验证命令：

```text
node_modules/.bin/vue-tsc.CMD -b --noEmit
node_modules/.bin/vitest.CMD run src/components/map/map-native-roads.test.ts src/components/map/map-sweep.test.ts
```

最近一次结果为 TypeScript 通过、道路相关测试 7/7 通过。
