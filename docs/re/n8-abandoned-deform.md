# N8：废弃建筑歪斜——引擎形变系统完整逆向（2026-10-01）

> 来源：shader 容器 0x0469A3F7（SimCity_App.package，32 个/37MB）命名片段解析。
> 全部源码为引擎原文（HLSL vs_3_0），提取脚本 `tmp/parse_container_tokens.py`，
> 逐字未改写。状态：**数学完整，可直接实现进 OpenSCP 渲染器**。

## 一、结论速览

废弃建筑"歪歪斜斜"是**引擎级顶点形变**，由三套机制叠加：

1. **正弦弯曲**（`building5MorphRotationAnimVS` 内 kDeform 循环）——歪斜主因；
2. **平面塌陷**（`deformAbandonedVS`）——沿 4 片斜切平面向内推挤；
3. **倒塌动画**（`animMorphRot`）—— destroyed 建筑的平移/旋转倒塌时间线。

三者均为**每实例数据驱动**（batchInstanceInfo 常量数组），非 mesh 替换。

## 二、正弦弯曲（歪斜主因）

```hlsl
// building5MorphRotationAnimVS（引擎原文）
for (int i = 0; i < kNumDeforms; ++i) {
    float len    = dot(Current.position.xyz, kDeformDir[i]);
    float amount = kDeformAmplitude[i] * sin(len / kDeformWavelength[i]);
    Current.position.xyz += amount * kDeformAxis[i];
}
```

- 每实例 kNumDeforms 组参数：`kDeformDir`（投影方向）、`kDeformAxis`（位移轴）、
  `kDeformAmplitude`（振幅）、`kDeformWavelength`（波长）；
- 效果 = 沿 dir 投影长度做**正弦弯曲**，弯向 axis——这就是废弃楼"歪歪斜斜"的
  视觉来源（蛇形/倾斜可由不同 axis 组合出）。

## 三、平面塌陷（deformAbandonedVS）

```hlsl
float planeOff = 0;
int maxPlane = 0;
float maxPlaneDist = dot(Current.position, float4(kPlanes[0].xyz, planeOff)) * kOffs[0];
for (int i = 1; i < kNumPlanes; ++i) {
    float thisPlaneDist = dot(Current.position, float4(kPlanes[i].xyz, planeOff)) * kOffs[i];
    if (thisPlaneDist < maxPlaneDist) { maxPlane = i; maxPlaneDist = thisPlaneDist; }
}
Current.position.xyz += kMove * -max(maxPlaneDist, 0.0) * 0.99;

static const int   kNumPlanes = 4;
static const float4 kPlanes[4] = {
    float4(normalize(float3(0, 0.2, 1)), 1),
    float4(normalize(float3(0, -0.2, 1)), 1),
    float4(normalize(float3(0.2, 0, 1)), 1),
    float4(normalize(float3(-0.2, 0, 1)), 1)
};
static const float3 kMove = float3(0, 0, 1);
static const float  kOffs[4] = {
    1.0/dot(kPlanes[0].xyz, kMove), 1.0/dot(kPlanes[1].xyz, kMove),
    1.0/dot(kPlanes[2].xyz, kMove), 1.0/dot(kPlanes[3].xyz, kMove)
};
```

- 4 片平面全部朝 +Z 带 ±0.2 倾角：把建筑"往里压"；
- `kOffs = 1/dot(plane, kMove)` 归一化，使位移沿 kMove 度量；
- 顶点在任一平面**后方**（距离为负）时沿 −kMove 推回 `0.99×深度`——
  即"超过立面的部分塌进去"，与废弃建筑的破口/凹陷观感一致。

## 四、配套：瓦砾重着色与开关

```hlsl
// deformRubbleVS：瓦砾化 = 实例色 2.2 次幂（提亮烧过的灰烬色）
Current.color = float4(pow(instanceColor.xyz, float3(2.2, 2.2, 2.2)), 1);

// setAbandoned / nonAbandoned：编译期开关
static const bool isAbandoned = false;   // 两份编译变体由此切换
```

- `isAbandoned` 切换 deform 系与非 deform 系编译变体——**触发源为
  建筑实例的废弃状态位**（存档/模拟数据），OpenSCP 可直接从 lot 的
  building 实例读取。

## 五、倒塌动画（同族，供参考）

```hlsl
void animMorphRot(float4 axisAnim, float4 centerAngle, inout float3 pos,
                  inout float3 normal, inout float3 tangent, float4 animTimes)
```

- 每实例数据存于 `batchInstanceInfo[48]`（`{translation, rotation, animdata}`）；
- 量化格式：平移 ±512 → `×1/64`；旋转轴 0..254（127=零）→ `×1/127−1`；
  角度半角制 `×16/32768`；
- 平移模式：`pos += center.xyz × t`；旋转模式：绕 center 按
  `halfTheta = centerAngle.w × t` 四元数旋转（法线/切线同步旋转）。

## 六、OpenSCP 实现指引

1. **three.js**：`material.onBeforeCompile` 注入正弦弯曲段（常量用
   InstancedBufferAttribute 每实例传入），或直接 ShaderMaterial；
2. **废弃判定**：读 lot building 实例的废弃状态位 → 选择 deform/非 deform
   编译变体（与引擎 `setAbandoned/nonAbandoned` 同构）；
3. **参数来源**：kNumDeforms/kDeformDir/Axis/Amplitude/Wavelength 为每实例
   数据——OpenSCP 可从存档 building 实例读，或按"废弃等级"程序化生成；
4. **瓦砾色**：`pow(instanceColor, 2.2)` 同款伽马即可复现烧灼色。

## 七、来源与验证

- 容器：`SimCity_App.package` g40212002（HLSL 明文），提取脚本
  `tmp/parse_container_tokens.py`（长度前缀字符串流解析）；
- 完整源码：`tmp/dynamic/all_blocks_index.txt`（6636 块索引）+
  `tmp/dynamic/N8_*.hlsl`；
- 交叉验证：`deformAbandonedVS` 与博客第 8 节"渲染走 RW4 设备抽象"互证
  （形变发生在引擎自己的着色器里，不经过 d3d9 固定管线）。
