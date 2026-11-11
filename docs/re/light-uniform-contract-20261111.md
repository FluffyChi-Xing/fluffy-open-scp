# 游戏光源结构体契约（从 App.package 提取的渲染器 HLSL 实证）

来源：`SimCity_App.package`，资源 type `0x0469A3F7`，group `0x40212002` / `0x40212012`，
instance 0–3（不同着色档）。提取产物：`tmp/shader-dump/*.hlsl`（git 忽略），
提取器：`src-tauri/examples/extract_shaders.rs`。

容器格式：`HLSL 片段 + NUL + uniform 声明表`（表项 = u32BE 名字长度 + 名字 +
u16 a / u16 b / u16 c / u16 pad）。本文件 509 条 uniform。

---

## 1. 每盏灯的字段（HLSL 反编译代码直接读出）

```hlsl
float3 p0   = mul(float4(deferredLight.position.xyz,    1), worldToCamera);
float3 p1   = mul(float4(deferredLight.endPosition.xyz, 1), worldToCamera);
float  radius      = deferredLight.position.w;       // 外半径
float  radius2     = radius * radius;
float  innerRadius = deferredLight.endPosition.w;    // 内半径
float  innerRadius2= innerRadius * innerRadius;
float  Kd          = deferredLight.params.x;         // 漫射（diffuse）
float  Ks          = deferredLight.params.y;         // 高光/反射（specular）
float  falloffStart= deferredLight.params.z;         // 衰减起点
float  falloffDist = deferredLight.params.w;         // 衰减距离
float  denomFalloff= 1.0 / (falloffDist - falloffStart + 0.0001);
float  denomCone   = 1.0 / (innerRadius - radius    + 0.0001);
float4 level       = float4(Kd, Kd, Kd, Ks);
```

**映射到我们作者侧的属性（逐项一一对应）**：

| 英雄属性（LOTM/scene） | 打包位置 | shader 语义 |
|---|---|---|
| `outerRadius` | `position.w` | `radius`（照亮半径，**也是 Line 段的半厚**） |
| `innerRadius` | `endPosition.w` | `innerRadius`（核心亮区；与 radius 一起算 `denomCone`） |
| `diffuse` | `params.x` | `Kd`（漫射强度，乘灯光颜色） |
| **`specular`** | `params.y` | **`Ks` = 用户说的"反射"** |
| `falloffStart` | `params.z` | 衰减起点（当前编辑器**完全没有**这个字段的语义实现） |
| `falloffDist` | `params.w` | 衰减距离 |
| `length` | `endPosition.xyz`（Line） | 条形灯的段长/方向 |
| `cullDistance` | 不打包进 shader | **CPU 侧按 Near/Mid/Far/Max 档位决定是否提交这盏灯** |

两条归一化分母解释了此前的观测：

- `denomCone = 1/(innerRadius − radius)`：**内半径必须 ≤ 外半径**，否则符号翻转、锥体计算失真
  （这正是"锥形光在地上投不出圆形光斑"的一个来源）。
- `denomFalloff = 1/(falloffDist − falloffStart)`：**falloffDist 必须 > falloffStart**，
  否则同样翻转。编辑器当前用 three.js 的 `decay=1` + `distance=radius*2`，**没有这两条约束**。

## 2. 高光（反射）的取值范围

`Ks` 直接进入 `level.w`，与 `Kd` 同量纲；代码里没有额外归一化，随后乘到
`shColorSpec` 一类高光项上。因此：

- 编辑器里 `specular` 应作为**独立的强度因子**参与高光，而不是折算进漫射；
- 上限由 UI 约束（建议与 `diffuse` 同范围，因为同一 `params` 四元组）；
- 之前"反射开到 5，编辑器极亮、游戏几乎不亮"的原因：编辑器把 `diffuse × 16` 全量算进
  漫射光照，而游戏是 `Kd` 走漫射、`Ks` 走高光两项独立通道 —— 两者尺度完全不同。

## 3. 体积光

`volumeInfo / volumeUVW / volumeCameraPosition / volumeCameraPosition` 一族用于
体积通道；`flatteningExpansion`（float4）与锥体展开相关。编辑器当前自造的
`lightVolumeIntensity(diffuse, volumetricStrength)` 与它们无对应关系，需要按
`radius/innerRadius + falloffStart/falloffDist` 重写。

## 4. 裁切（回答"按距离裁切"）

shader 侧**没有**距离裁切 uniform；`cullDistance` 是数据侧四档 Key
（`CULL_NEAR/MID/FAR/MAX`，见 `scene.rs:631`），由 CPU 决定是否把这盏灯写进
deferred light 列表。所以编辑器正确做法 = 按档位 + 相机距离决定 `visible`，
而不是限制灯的**总数**。

## 5. 待办

1. `overlays.ts` 按 §1 的公式重写：`inner ≤ outer`、`falloffDist > falloffStart`
   两条约束前置校验；`diffuse→Kd`、`specular→Ks` 分开使用；
2. `cullDistance` 四档接入渲染循环（可见性），删除任何"固定 N 盏灯"的做法；
3. 体积光按 `radius/innerRadius/falloff*` 重算；
4. 属性面板已加语义提示（`FAlert` + `package.lightSemantics*`），后续把
   §1 的取值约束（内≤外、falloffDist>falloffStart）也显示出来。
