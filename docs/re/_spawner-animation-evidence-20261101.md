# SimCity 2013 小人/车辆 渲染期动画取证（dev build 反编译源码）

- 取证时间：2026-11-01
- 源根：`D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\<Namespace>\<Class>\<Function>.c`
- 结构体：`D:\ea-games\simcity_dev\new-cource\SimCity.unwrapped.exe.dec\SimCity.unwrapped.exe.dec\all_types.h`
- 单文件总反编译：`D:\ea-games\simcity_dev\SimCity.unwrapped.exe.c`（71 MB，仅定点 grep 读取）
- 全程只读；本文件是唯一写入产物。
- 引用格式：`相对源根的路径:行号`。所有引用块均为**逐字**反编译文本（含反编译器的 `this:` / `a2:` 命名实参扩展）。
- 记号：**【事实】** = 代码字面可读；**【推理】** = 由代码+结构体算术推出但无单行字面；**【数据侧】** = 由既往对游戏数据（属性表/资源）的提取交叉印证，不在本次 C 代码内；**【未定】** = 现有材料无法判定。

---

## 0. 链路总览（先给结论地图）

crowd 小人（你在大街上看到的那种）**不是**从 3D 模型直接画出来的，而是：

1. 属性表 → `cGraphicsInstancedSim`（外观选项表 + impostor type id）；
2. 每帧 `cGraphicsGame::UpdateTransports` → `GetSimData` 生成 `modelFlag`（64 位部件掩码）与 `simInfo`（调色板索引），写入 impostor 数据缓冲；
3. 每帧 `cImpostorRenderer::Update` 推进每个**部件**（body/head 各是独立 `SP::cModelInstance`）的 `cModelInstanceAnimations`，随后 `RenderOffscreen` 把这些部件以 1..8 个方位角**实例化重绘**进 512×512 图集；
4. 每帧 `RenderParticles` 把每个小人展开成 **2 个四边形**（body 槽 1 个 + head 槽 1 个），每个四边形 4 顶点，顶点里塞 `(scale, 部件号, 方位帧号)`，由像素着色器按部件号取图集矩形、按方位帧号偏移 U、按实例色 R/G 查两张调色板。

即：**小人 = 预渲染图集公告板（impostor）+ 2 个刚性部件槽 + 单一朝上轴（yAxis）朝向 + 速度阈值选姿态**；真正有骨骼关键帧动画的是"部件的离屏重绘"，而不是每个小人各有一条动画时间线。

---

## 1. 小人是蒙皮网格还是刚性拼装部件？`modelFlag` 逐位解码

### 结论

**刚性拼装（composite rigid parts），且最终呈现为图集公告板。** `GetSimData` 输出的 64 位 `modelFlag` 是一张"部件开关表"，每个置位 bit = 一个**独立的 `SP::cModelInstance` 部件**（自带模型资源 + 自带一条动画 + 自带图集矩形）。一个小人常态只置 **2 个 bit**：一个 body 部件 + 一个 head 部件（头并不通过骨骼挂接，而是另一个被同一投影框渲染的独立矩形，见 §2）。

逐位布局（**【推理】**，由 `+9` 与 `3*body+walk` 的算术唯一确定）：

| bit 区间 | 含义 | 依据 |
| --- | --- | --- |
| `0..8` | body 部件，共 **9** 槽 = `3 个 body 外形 × 3 个姿态槽`（步长 3） | `body + (walk?1:0) + 2*body` = `3*body + walk`，body∈{0,1,2} 时覆盖 0..8 |
| `9..` | head 部件，`head` 从 0 起，bit = `head + 9` | `1LL << (head + 9)` |
| 每 body 姿态槽 | 槽 0 = **idle（站立）**，槽 1 = **walk（行走）**，槽 2 = 本函数从不使用 | `+ (speed² > kSimIdleSpeed2)` 只产出 0/1 |

`kSimIdleSpeed2` 的取值是 **`0.010000001`（float 常量，位于 `.rdata`）**，且比较对象是速度的**平方和**，因此阈值等价于速度模长约 **0.1 单位/秒**（1/10 m/s 量级）——这就是"站着"与"走路"的分界。整个阈值判定**没有计时器、没有相位累加器、没有迟滞**。

### 证据

`src\SC\cGraphicsInstancedSim\GetSimData.c:100-107` —— 唯一的 `modelFlag` 产生点：

```c
  *modelFlag = (1LL << (head + 9))
             | (1LL << (body
                      + ((float)((float)((float)(velocity.z * velocity.z) + (float)(velocity.y * velocity.y))
                               + (float)(velocity.x * velocity.x)) > kSimIdleSpeed2)
                      + 2 * body));
  *scale = (float)((float)((float)((float)(maxHeight - minHeight) * (float)v7) * 0.06666667) + minHeight) * *scale;
  *simInfo = (outfit | ((unsigned __int8)randomBits << 8)) << 8;
```

`D:\ea-games\simcity_dev\SimCity.unwrapped.exe.c:170402` —— `kSimIdleSpeed2` 的定义（同一数据段内紧邻 `kSPTwoPi_273`、`kSPXAxis3_273`，而 `kSPZero3_273` 正是 `GetSimData` 另一调用点 `cUnitBinDrawResources::AddInstanceToDrawList` 传入的速度值，可证同翻译单元）：

```c
const float kSimIdleSpeed2 = 0.010000001; // idb
```

`src\SC\cGraphicsInstancedSim\GetSimData.c:44-53` —— 选项索引与随机位切分（`randomBits` 的位域布局在这里被完整读出）：

```c
  mNumSimOptions = this->mNumSimOptions;
  v7 = BYTE1(randomBits) & 0xF;
  ...
    v8 = (randomBits >> 12) % mNumSimOptions;
```

`src\SC\cGraphicsInstancedSim\GetSimData.c:91-96` —— 唯一的范围守卫（注意它就是**松的**，见"未定"）：

```c
    if ( mBody <= 3u && mHead <= 0x32u )
    {
      body = mBody;
      head = mHead;
      outfit = mOutfitPalette;
    }
```

`src\SC\cGraphicsInstancedSim\GetSimData.c:72-88` —— 混合进制展开（把 `v8` 还原成 body/head/outfit，说明外观表每行给的是**区间基址+数量**）：

```c
        v12 = mpBegin->mNumHeads * mpBegin->mNumBodies * mpBegin->mNumOutfits;
        if ( v8 < v12 )  break;
        ++v9; v8 -= v12; ++mpBegin;
      ...
      v14 = v8 % mpBegin->mNumBodies;
      v13 = v8 / mpBegin->mNumBodies;
      mNumHeads = mpBegin->mNumHeads;
      mBody = v14 + mpBegin->mBody;
      mOutfitPalette = v13 / mNumHeads + mpBegin->mOutfitPalette;
      mHead = v13 % mNumHeads + mpBegin->mHead;
```

`all_types.h:137565-137570` —— 承载者只有三个字段（没有逐实例动画状态可放）：

```c
struct __cppobj SC::cGraphicsInstancedSim
{
  unsigned int mImpostorType;
  eastl::vector<SC::cSimGraphicsInfo,eastl::fixed_compat_allocator> mSimInfo;
  unsigned int mNumSimOptions;
};
```

`all_types.h:137536-137548` —— 外观行结构（`mHead/mNumHeads/mBody/mNumBodies/mOutfitPalette/mNumOutfits/mMinScale/mMaxScale`）：

```c
  unsigned __int8 mHead;
  unsigned __int8 mNumHeads;
  unsigned __int8 mBody;
  unsigned __int8 mNumBodies;
  unsigned __int8 mOutfitPalette;
  unsigned __int8 mNumOutfits;
  float mMinScale;
  float mMaxScale;
```

`src\SC\cGraphicsInstanced\FillFromProps.c:252-284` —— 外观表装配（8 列属性，`mNumSimOptions` 是**所有行选项数之积的和**，不是行数；这正好解释 `GetSimData` 里那个"行数够不够"的分支）：

```c
      v12 = SP::cSimpleUnion<536,8>::Create<SC::cGraphicsInstancedSim>(this: &this->mData);
      *(_DWORD *)v12->buffer = impostorType;
      ...
      eastl::vector<SC::cSimGraphicsInfo,eastl::fixed_compat_allocator>::resize(
        this: (eastl::vector<SC::cDestructionParcel,eastl::fixed_compat_allocator> *)&v12->buffer[4],
        n: PropertiesAsTable);
```

`src\SC\cGraphicsInstanced\FillFromProps.c:340-347`：

```c
          LODWORD(impostorDataViewType) += v17;   // v17 = numBodies*numHeads*numOutfits
          ...
      *(float *)&v12->buffer[20] = impostorDataViewType;   // mNumSimOptions
```

**部件上限**：`all_types.h:74599-74602` 与 `74642` 表明 `mParts` 是 `fixed_vector<cImpostorClassPart,64>`；`src\SC\cImpostorRenderer\InitImpostorClass.c:292-295` / `:310-312` 把部件数从属性表行数夹到 `[0,64]`：

```c
    if ( v25 <= 64 )
      goto LABEL_36;
    v26 = 64;
  ...
  pClass->mParticleData.mNumParticleTypes = v26;
  pClass->mShadowParticleData.mNumParticleTypes = v26;
```

**【数据侧】交叉印证"9 + 20 = 29"**：既往对 `22706EFA`（小人 impostor class）属性表的提取给出 **29 个 part、8 方位角、身体 3 型 / 头 20 型**（`docs\re\props-effects-spawners-paths-engine-flow.md:364-377`）。9 个 body 槽 + 20 个 head 槽 = 29，与 `head + 9` 的偏移、`3*body` 的三槽步长完全自洽。

### 未定 / 反证

- **`+9` 与 `2*body` 没有任何符号常量证据**：全树 grep `kSimIdleSpeed` 只命中 `GetSimData.c` 一处；`9` 与 `3` 都是裸字面量。若原始源码写的是 `kNumBodyParts`/`kNumBodyPoses`，其定义在头文件里、未进入函数级反编译树。**"9 = 3 body × 3 姿态"是算术结论 + 数据侧 29 part 的印证，属推理，不是字面事实。**
- **守卫与编码互相矛盾（真实反证）**：守卫允许 `mBody == 3`，但 `3*3+{0,1} = 9,10` 正好撞上 `head + 9` 的 head 0 / head 1。也就是说若数据里真的存在第 4 个 body 外形，body=3 的小人会随机"丢头/换头"。数据侧只有 3 型身体，所以现实中不会触发——但这是**编码位宽不足**的硬矛盾，不能解释成"4 个 body"。
- **每 body 的第 3 个姿态槽（`3*body+2`）用途完全未知**：本函数只产出 +0/+1。可能在别的（未进入本树的）代码路径里用于"坐下/驾驶/搬运"等，也可能纯属留空。**无证据。**
- **`.06666667` = 1/15**：缩放抖动取 `BYTE1(randomBits)&0xF ∈ [0,15]`，乘 1/15 后正好铺满 `[minScale, maxScale]`。这点是字面可读的，但"为什么要 16 档"没有依据。
- `randomBits` 位域布局【事实】：`[0..7]` = 肤色/皮肤调色板索引（`GetSimPaletteData.c:34` `*skin = randomBits;`）、`[8..11]` = 体型缩放抖动、`[12..]` = 外观组合序号。三者在同一 32 位里"挤"在一起，说明外观完全由 agent 的稳定随机 id 决定，**没有任何逐实例动画状态位**。

---

## 2. `modelFlag` 的下游消费：从掩码到 2 个四边形

### 结论

`modelFlag` 一路以 `unsigned __int64` 身份流到 `cImpostorType::mModelEnabledFlags`，逐帧被"部件总数掩码"裁剪后**数 1 的个数**决定要写多少个四边形；写顶点时用 `_BitScanForward` **从低位到高位**取出 bit 号，该 bit 号**就是部件号**，被写进顶点法线通道 `mN3F.y`。

- 顶点布局：`cVertex_V3F_N3F_C4B_T2F`，stride 36；
- 每个部件 → 1 个四边形 → 4 个顶点，4 顶点**位置完全相同**（世界坐标），靠 `mT2F.x ∈ {0,1,2,3}` 传给顶点着色器做"角点号"，由着色器展开成面向相机的卡片；
- `mN3F.x` = `scale * mScaleFactor`（float 位塞进法线槽）、`mN3F.y` = **部件号**、`mN3F.z` = **方位帧号**（0..numAngles-1）、`mT2F.y` = 挂接点索引（`mAttachPoints[部件号]`，无挂接为 -1）。

"动画帧"在这一层**只有方位帧**（`mN3F.z`），没有时间帧——时间维的动画已经在图集里被"重绘"消费掉了（§2.3）。

### 证据

#### 2.1 缓冲区分配与掩码存放（`GetDataBuffer`）

`src\SC\cImpostorRenderer\GetDataBuffer.c:84-91` —— 五条并行数组：位置/朝向/缩放/颜色(simInfo)/64 位掩码：

```c
  p_mHasOwnership = (eastl::vector<unsigned __int64,eastl::fixed_compat_allocator> *)&second[6].mHasOwnership;
  eastl::vector<unsigned __int64,eastl::fixed_compat_allocator>::resize(this: p_mHasOwnership, n: v14, &value);
  v18 = &v21->__vftable;
  *ppRenderFlags = &p_mHasOwnership->mpBegin[v13];
  v18[226] = ((int)impostorType - v18[3]) >> 3;
  result = numImpostors;
  v18[227] = numImpostors;
```

（`second` 是 `cImpostorType`；反编译器把它的 `mPositions/mDirections/mScales/mData/mModelEnabledFlags` 误当成了 `cWebViewJavaScriptValueRef` 的成员，字段顺序与 `all_types.h:74658-74675` 一一对应。`v18[226]` = `mCurrentImpostorTypeIndex`，`v18[227]` = `mCurrentDataBufferAllocationSize`。）

`all_types.h:74658-74675` 与 `all_types.h:74785-74798`：

```c
struct __cppobj SC::cImpostorType
{
  EA::AutoRefCount<SP::cPropertyList> mConfig;
  unsigned int mId;
  SC::cImpostorClass *mClass;
  unsigned int mPersistenceClassId;
  eastl::vector<cSPVector3,eastl::fixed_compat_allocator> mPositions;
  eastl::vector<cSPVector3,eastl::fixed_compat_allocator> mDirections;
  eastl::vector<float,eastl::fixed_compat_allocator> mScales;
  eastl::vector<unsigned int,eastl::fixed_compat_allocator> mData;
  eastl::vector<unsigned __int64,eastl::fixed_compat_allocator> mModelEnabledFlags;
  float mScaleFactor;
  SP::cMaterial *mParticleMaterial;
  bool mVBDirty;
  ...
};
struct __cppobj SC::cImpostorRenderer : EA::Messaging::IHandler
{
  SC::cImpostorClass *mClasses[2];
  eastl::vector_map<unsigned int,SC::cImpostorType *,...> mTypes;
  ...
  SP::cRectAllocator mAllocator;              // 512×512 图集矩形分配器
  SC::cShaderDataImpostorAtlasPosition mAtlasInfo;
```

#### 2.2 掩码 → 四边形数量（`BuildVertexBufferForType`）

`src\SC\cImpostorRenderer\BuildVertexBufferForType.c:105-113`：

```c
      v7 = (char *)pType->mClass->mParts.mpEnd - (char *)pType->mClass->mParts.mpBegin;
      vb = (rw::graphics::VertexBuffer *)pType->mClass;
      numParts = v7 >> 6;                                  // sizeof(cImpostorClassPart) == 64
      mpBegin = pType->mModelEnabledFlags.mpBegin;
      lockFlags = (unsigned __int64)((1LL << (v7 >> 6)) - 1) >> 32;
      v10 = (1 << (v7 >> 6)) - 1;                          // 低 32 位"部件存在"掩码
      ...
      locked.lockFlags = lockFlags;                        // 高 32 位同上
```

`src\SC\cImpostorRenderer\BuildVertexBufferForType.c:215-226`（≤32 个部件时的 popcount 快路径，逐实例累加）与 `:228`：

```c
            v14 = mpBegin[v13 + 1];
            jobData.mpObject = (cRenderParticlesBatchJobInfo *)((char *)jobData.mpObject
                                                              + cBitsSetTable256[v10 & mpBegin[v13]]);
            numParticlesInThisBatch += cBitsSetTable256[v10 & v14];
      ...
      numBatches = (numParticles + 2045) / 0x7FE;          // 每批 ≤2046 个四边形
```

`src\SC\cImpostorRenderer\BuildVertexBufferForType.c:277-301` —— 把所有指针与掩码塞进批信息（`mParticleFlagMask` = 部件存在掩码）：

```c
  p_mData->mNumParts   = v31;
  p_mData->mNumAngles  = (int)m_d3dVB;
  p_mData->mNumImpostors = pType->mPositions.mpEnd - pType->mPositions.mpBegin;
  p_mData->mCosAngleTable = (float *)&v30[416].m_usage;
  LODWORD(p_mData->mParticleFlagMask) = v10;
  ...
  p_mData->mFacings = pType->mDirections.mpBegin;
  p_mData->mScales  = pType->mScales.mpBegin;
  p_mData->mColorData = pType->mData.mpBegin;
  p_mData->mPtrModelFlags = pType->mModelEnabledFlags.mpBegin;
  p_mData->mStride = 36;
```

#### 2.3 顶点生成：部件号 + 方位帧号（`RenderOneParticleBatch`）

`src\global\RenderOneParticleBatch.c:118-126` —— 取最低置位 bit，**bit 号即部件号**：

```c
      v14 = !_BitScanForward(&v15, mModelFlags);
      if ( v14 )
      {
        v14 = !_BitScanForward((unsigned int *)&v16, mModelFlags_high);
        if ( v14 )
          v15 = 64;
        else
          v15 = v16 + 32;
      }
```

`src\global\RenderOneParticleBatch.c:127-140` —— 4 顶点共用位置，法线槽装 `(scale, 部件号, 方位帧)`：

```c
      x = pRPBI->mPositions->x;
      y = pRPBI->mPositions->y;
      z = pRPBI->mPositions->z;
      mAttachPoint = (float)pRPBI->mAttachPoint;
      mVertex->mV3F.x = x;
      modelData_4 = (float)v15;
      mVertex->mV3F.y = y;
      mVertex->mV3F.z = z;
      mVertex->mC4B = v6;
      mVertex->mN3F.y = modelData_4;      // 部件号
      mVertex->mN3F.x = v8;               // scale*mScaleFactor
      mVertex->mN3F.z = v13;              // 方位帧号
      mVertex->mT2F.x = 0.0;              // 角点号
      mVertex->mT2F.y = mAttachPoint;     // 挂接点
```

`src\global\RenderOneParticleBatch.c:141-171` —— 其余 3 顶点的 `mT2F.x` 依次为 1.0 / 2.0 / 3.0（位置完全不变）：

```c
      mVertex[1].mT2F.x = 1.0;
      ...
      v21->mT2F.x = 2.0;
      ...
      v21->mT2F.x = 3.0;
```

`src\global\RenderOneParticleBatch.c:172-178` —— 消费掉一个 bit，并把"挂接点"换成该部件登记的挂接点：

```c
      pRPBI->mAttachPoint = pRPBI->mAttachPoints[v15];
      mVertex = v21 + 1;
      v22 = (mModelFlags != 0) + mModelFlags_high - 1;
      mModelFlags &= mModelFlags - 1;          // 清最低位
      mModelFlags_high &= v22;
      if ( (mModelFlags_high | mModelFlags) == 0 )
        goto LABEL_21;
```

`src\global\RenderOneParticleBatch.c:103-104` —— 下一实例的掩码也要与"部件存在掩码"相与：

```c
          mModelFlags = *(_DWORD *)mPtrModelFlags & pRPBI->mParticleFlagMask;
          mModelFlags_high = *((_DWORD *)mPtrModelFlags + 1) & HIDWORD(pRPBI->mParticleFlagMask);
```

#### 2.4 部件矩形、方位帧、动画速度（`InitImpostorClass` / `Update`）

`src\SC\cImpostorRenderer\InitImpostorClass.c:171-176` —— `mNumAngles` 来自属性 id `198583670`（`0x0BD6257E`），夹到 **[1,8]**：

```c
  if ( pProps != nullptr && pProps->GetProperty_2(this: pProps, a2: 198583670, a3: &v83) != nullptr && v83->mTypeId == 9 )
    pClass->mNumAngles = *EA::Variant::operator<int> int &(this: v83);
  if ( pClass->mNumAngles > 8 )
    pClass->mNumAngles = 8;
  if ( pClass->mNumAngles < 1 )
    pClass->mNumAngles = 1;
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:193-204` —— 每个方位一条"角度常量顶点"，并算角步长与着色器寄存器数：

```c
  for ( i = 0; i < pClass->mNumAngles; v8 += stride )
  {
    *v8 = i;
    v8[1] = i;
    v8[2] = i;
    v8[3] = i++;                 // 4 个 ubyte 全写同一个角度号
  }
  ...
  v11 = (float)(kSPPi_238 * 2.0) / (float)mNumAngles;
  pClass->mShaderAngles.mNumRegisters = (mNumAngles + 1) / 2;
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:241-256` —— 朝向查表的**建表**（`mCosAngleTable[i] = ±(cos θ+1)`，θ=(i+0.5)·2π/N；`sin>0` 取负；末尾哨兵 `FLT_MAX`）：

```c
      *(float *)&v83 = (float)((float)v20 + 0.5) * v11;
      v22 = *(double *)__libm_sse2_sin().m128_u64;
      v23 = *(double *)__libm_sse2_cos().m128_u64;
      v24 = v23 + 1.0;
      if ( v91 > 0.0 )
        LODWORD(v24) ^= _mask__NegFloat_;
      *mCosAngleTable = v24;
  ...
  pClass->mCosAngleTable[pClass->mNumAngles] = 3.4028235e38;
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:359-363` —— 部件矩形宽度 = **单帧宽 × 方位帧数**（横向拼 N 帧）：

```c
      v39 = SP::cRectAllocator::AddRect(
              this: &v92->mAllocator,
              w: (_DWORD)v83 * pClass->mNumAngles,
              h: *(_DWORD *)(v103 + 4 * v89));
      v29->mRectID = v39;
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:491-492`、`:506`、`:518-526` —— 每部件三件套：**动画速度**、**子骨骼**、**复合顶点声明**（网格 VB + 角度 VB）：

```c
          LODWORD(v29->mAnimSpeed) = v59;                    // 属性缺省 1.0（见 :490 = 1065353216）
          ...
          v29->mChildBoneIdx = -1;
          ...
          descs[0] = (*mpBegin)->m_vertexBuffer[0]->m_desc;
          descs[1] = mAngleVertexDescriptor;
          v29->mInstancingDescriptor = SP::CombineVertexDescriptorsAsStreams(descs, numDescs: 2);
          ...
            v29->mChildBoneIdx = `anonymous namespace'::FindBoneIndex(pAnims: v84, boneName: v69);
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:529-542` —— 图集矩形写成纹理变换（**1/512** 归一化，证明图集尺寸 512×512）：

```c
          mLeft = v70->mRect.mLeft;
          locked.stride = v70->mRect.mTop;
          mBottom = v70->mRect.mBottom;
          ...
          v74 = (float)(int)locked.stride * 0.001953125;      // 1/512
          v76 = (float)mLeft * 0.001953125;
          v77 = (float)((float)(mLeft + v72) * 0.001953125) - v76;
          v78 = (float)(v73 * 0.001953125) - v74;
```

`src\SC\cImpostorRenderer\Update.c:41-49` —— **逐帧推进每个部件的动画**（`dt × mAnimSpeed`），这就是小人"走路/呼吸"的时间来源：

```c
        mpBegin = v4->mParts.mpBegin;
        mpObject = mpBegin[v5].mAnimation.mpObject;
        v9 = &mpBegin[v5];
        if ( mpObject != nullptr )
          SP::cModelInstanceAnimations::Animate(this: mpObject, deltaTime: v9->mAnimSpeed * dt);
```

`src\SC\cGraphicsGame\Update.c:166` —— 每帧调用一次：

```c
    SC::cImpostorRenderer::Update(this: this->mImpostorRenderer, dt);
```

#### 2.5 离屏重绘 = 每部件 × 每方位，GPU 实例化

`src\SC\cImpostorRenderer\RenderOffscreen.c:96-111` —— 遍历 class 的两套（shadow / 非 shadow）× 所有部件：

```c
          nModels = v11->mParts.mpEnd - v11->mParts.mpBegin;
          ...
              v13 = &v11->mParts.mpBegin[v24];
              SC::cImpostorRenderer::RenderOffscreenForModel(
                this, isShadow, stats, pClass: v11, aggModel: v6, &camXform);
```

`src\SC\cImpostorRenderer\RenderOffscreen.c:59-77` —— 相机固定为"零平移 + 后旋 -90°（Z-up 修正）"，方位旋转由角度顶点承担：

```c
      camXform.mTranslation = kSPZero3_238;
      camXform.mScale = 1.0;
      ...
      cSPTransform::PostRotateX(this: &camXform, angle: kSPPi_238 * -0.5);
```

`src\SC\cImpostorRenderer\RenderOffscreenForModel.c:143-171` —— 按方位循环，累加外包盒（用于算卡片尺寸），逐方位算 `mAtlasInfo.mScale[i]/mOffset[i]/mTransform[i]`：

```c
  do
  {
    cSPTransform::cSPTransform(this: &v79, __that: (const EA::Swarm::cTransform *)camXform);
    ...
    AnalyzeBoundingBox(&out, bbox: &v9->mBox, xform: (const cSPTransform *)&v79);
```

`src\SC\cImpostorRenderer\RenderOffscreenForModel.c:248-256` —— 卡片数据（世界尺寸 + 深度 + 方位数），写入 `cShaderDataImpostorParticles`：

```c
  p_mShadowParticleData->mParticles[v30].mCardData.x = v12;
  p_mShadowParticleData->mParticles[v30].mCardData.y = v14;
  p_mShadowParticleData->mParticles[v30].mCardData.z = v25;
  p_mShadowParticleData->mParticles[v30].mCardData.w = v27;
  mZBias = v9->mZBias;
  p_mShadowParticleData->mParticles[v30].mCardDepths.w = (float)pClass->mNumAngles;
  p_mShadowParticleData->mParticles[v30].mCardDepths.y = mZBias;
  p_mShadowParticleData->mParticles[v30].mCardDepths.z = v28;
  p_mShadowParticleData->mParticles[v30].mCardDepths.x = v24;
```

`src\SC\cImpostorRenderer\RenderOffscreenForModel.c:263-271` —— **所有部件共享同一投影框**（这正是"同一四边形叠加 N 个矩形即可拼出完整小人"的机制）：

```c
  v6->mAtlasInfo.mProjScale.x = v32 * 2.0;
  v6->mAtlasInfo.mProjScale.z = v34;
  v6->mAtlasInfo.mProjScale.w = v36;
  v6->mAtlasInfo.mProjScale.y = v33 * 2.0;
  ...
  v6->mAtlasInfo.mProjOffset.x = (float)(v35 * 2.0) - 1.0;
  v6->mAtlasInfo.mProjOffset.y = y;
  v6->mAtlasInfo.mProjOffset.w = v37;
  v6->mAtlasInfo.mProjOffset.z = (float)-v24 * v34;
```

`src\SC\cImpostorRenderer\RenderOffscreenForModel.c:368-379` —— 真正的绘制：一次 `DrawBuffersInstanced`，`instanceCount = mNumAngles`，实例流 = 角度 VB：

```c
      SP::DrawBuffersInstanced(
        primType: datab->m_primType,
        vd: v84->mInstancingDescriptor,
        vb: v69,
        vertexStart: 0,
        vertexCount: v69->m_numVertices,
        ib: datab,
        indexStart: v60->m_start,
        indexCount: v67,
        vbi: v8->mAngleVB,
        instanceStart: 0,
        instanceCount: v8->mNumAngles);
```

`src\SC\cImpostorRenderer\RenderParticlesForType.c:56-82` —— 精灵绘制：只推三块着色器常量（角度表 / 部件表 / 挂接点表），然后按四边形索引缓冲 `6×count` 画：

```c
        SP::PushShaderData();
        pTexture = (SP::cTextureInstance *)"cImpostorRenderer::RenderParticles";
        m_d3dTexture = (char *)v6->m_d3dTexture;
        SP::SetShaderData(type: 0x2CEu, data: m_d3dTexture + 4412, dataChanged: false);
        ...
          SP::SetShaderData(type: 0x2C4u, data: m_d3dTexture + 4480, dataChanged: true);
          v12 = m_d3dTexture + 7556;
        SP::SetShaderData(type: 0x2CDu, data: v12, dataChanged: true);
        v14 = SP::cBufferDrawCommonData::mQuadIndexBuffer;
        ...
            v17 = 6 * *(_DWORD *)(*(_DWORD *)&v6[3].m_depth + 4 * v13);
              SP::Draw(state: v9, matInfo: mMaterialInfo, vb: v15, ib: v14, indexStart: 0, indexCount: v17);
```

`src\SC\RegisterShaderData.c:209-274` —— 三个块的身份（`0x2C4` 部件表、`0x2CD` 挂接表、`0x2CE` 角度表），以及 `0x2C9 = UploadSimPalette`：

```c
    dataID: 0x2C4u,  dataSize: 0xC04u, upload: UploadImpostorParticles,
    dataID: 0x2C9u,  dataSize: 0x14u,  upload: UploadSimPalette,
    dataID: 0x2CDu,  dataSize: 0x204u, upload: UploadImpostorAttachments,
    dataID: 0x2CEu,  dataSize: 0x44u,  upload: UploadImpostorAngles,
```

`src\global\UploadImpostorParticles.c:20-26` —— 每部件上传 **3 个 float4 寄存器**（即 `mTextureTransform/mCardData/mCardDepths`）：

```c
  v3 = (char *)SP::ShaderData(id: 0x2C4u);
  if ( v3 != nullptr )
  {
    v4 = size;
    if ( size > 3 * *(_DWORD *)v3 )
      v4 = 3 * *(_DWORD *)v3;
    SP::UploadFloatConstants(data: (cSPVector4 *)(v3 + 4), offset, size: v4, shaderType);
```

`src\SC\cGraphicsGame\Init.c:198-210` —— 两条渲染脚本命令（每帧由 render script 触发；图集不在 CPU 端保存为贴图，而是每次重绘）：

```c
  v39 = SP::RenderScriptManager();
  v39->RegisterCommand(this: v39, a2: "ImpostorOffscreen",
    a3: (void (__cdecl *)(...))SC::cImpostorRenderer::RenderOffscreenEntry,
    a4: this->mImpostorRenderer, a5: nullptr);
  v39->RegisterCommand(this: v39, a2: "ImpostorSprites",
    a3: (void (__cdecl *)(...))SC::cImpostorRenderer::RenderParticlesEntry,
    a4: this->mImpostorRenderer, a5: nullptr);
```

`src\SC\cGraphicsGame\UpdateTransports.c:465-479` —— 每实例写入 impostor 缓冲（朝向 = `mRotation.yAxis`）：

```c
                    v62 = ppDirections;
                    *(float *)((char *)&ppDirections->x + v60) = v55->mRotation.yAxis.x;
                    ...
                    v63[2] = v55->mRotation.yAxis.z;
                    v89.x = v56->mVelocity.x;
                    v89.y = v56->mVelocity.y;
                    v89.z = v56->mVelocity.z;
                    SC::cGraphicsInstancedSim::GetSimData(
                      this: p_mBuffer, velocity: v89, randomBits: v56->mRandomID,
                      modelFlag: &ppRenderFlags[v54], simInfo: &ppData[v54], scale: &ppScales[v54]);
```

`src\SC\cGraphicsGame\UpdateTransports.c:446-457` —— 速度是"本帧位移 / dt"，即**离散差分**（因此姿态切换最多滞后一帧）：

```c
                      v57 = v55->mTranslation.y - v56->mPrevPosition.y;
                      v58 = v55->mTranslation.z - v56->mPrevPosition.z;
                      v56->mVelocity.x = (float)(1.0 / gameSecondsElapsed)
                                       * (float)(v55->mTranslation.x - v56->mPrevPosition.x);
                      ...
                      v56->mPrevPosition.z = v55->mTranslation.z;
```

### 未定 / 反证

- **`mN3F.y` 里装"部件号"是反编译语义推断**：它只是一个 float 位模式，着色器侧如何用它索引 `cShaderDataImpostorParticles::mParticles[]`（按 `mNumParticleTypes` 夹取）属于 shader 文本，不在 C 树内。代码只保证"值 = 最低置位 bit 号"。
- **`cImpostorCell`（`all_types.h:125039-125049`）与建筑 impostor 是另一条链**：`cGraphicsGameImpostor::RenderOffscreen` 转调的是 `cUnitImpostorRenderer::RenderOffscreen`（`src\SC\cGraphicsGameImpostor\RenderOffscreen.c:34-39`），而不是 `cImpostorRenderer`。本次小人结论**不覆盖**建筑/道具的 unit-impostor 单元格机制。
- **`mModelEnabledFlags` 的掩码上界与"部件数 29"配合**：32 位路径（`:215-226`）在 `numParts ≤ 32` 时可用，小人 class 29 个部件正好落在该快路径；64 位路径（`:114-136`）只在部件更多时走。这不影响语义，但说明**小人是"部件数刚好卡在 32 位以内"的典型用例**。
- `GetDataBuffer` 的 `impostorType` 形参实际是**按值传入的 type id**（反编译签名写成了 `eastl::pair<...>*`，因为它既当 key 又当指针用，见 `:42-55` 的 `lower_bound` + `value: (unsigned int *)&impostorType`）。此处为**反编译签名失真**，不是真实 ABI。
- `src\SC\cUnitBinDrawResources\AddInstanceToDrawList.c:76-91` 有一处**参数顺序错位**（`simInfo:` 收到 `&scale`、`scale:` 收到 `&simInfo`），是反编译器对 `unsigned int*` / `float*` 交换实参的伪影；语义应以 `GetSimData` 的形参定义为准。

---

## 3. 同链路的其他动画机制：谁管建筑、谁管小人

### 结论

按"消费者"归属，可以干净地劈成三组：

| 机制 | 归属 | 作用 |
| --- | --- | --- |
| `cModelInstanceAnimations`（`SP::`）| **小人/树/道具的 impostor 部件** | 部件自带的骨骼动画；`cImpostorRenderer::Update` 用 `dt×mAnimSpeed` 推进，`RenderOffscreen` 每帧重绘进图集 |
| `cAnimModel`(+`cBinAnimInfo`) / `mAnimModelInfo` / `cChoreographyAnimations` / `MoveToTime(Relative)` | **建筑（lot 单元）真实关键帧动画** | 由经济资源 bin 的 `mAmount/mCapacity` 驱动一个 0..1 归一化时间，映射到动画片段区间；确有真关键帧 |
| `cSimpleAnims` / `cSimpleAnimJoint` / `cSimpleAnim` | **单元网格批处理（建筑/近景网格）的 CPU 蒙皮** | `cGraphicsUnitMeshBatcher → cGraphicsMeshBatch → cGraphicsSimpleMesh` 把关节变换烘进实例顶点 |
| `cAnimatingParticleEffect` | **粒子特效**（烟火/烟尘等） | 与小人无关 |
| `cAnimNametable` / `cDrivenKeys` | **RW4 动画数据类型的 arena fixup** | 本 exe 内只剩 `Fixup/Refix/Register*Callback`，无任何动画消费方 |

**因此任务里的命题成立且有代码支撑**：建筑（lot facade/单元）**有**真正的关键帧动画（且由模拟资源量驱动），小人**没有**逐实例关键帧——小人的时间维动画是"部件图集每帧重绘"的全局共享产物（§2.4/§2.5），逐实例只有"选哪个姿态槽 + 哪个方位帧"。

### 证据

#### 3.1 建筑：真关键帧 + 资源量驱动

`src\SC\cUnitModel\UpdateModelAnimations.c:58-87` —— 用资源 bin 比例算时间，`MoveToTimeRelative`：

```c
  mAnimResourceBinIndex = this->mAnimResourceBinIndex;
  ...
      mAnimValue = this->mAnimValue;
      v10 = (float)v8->mAmount / (float)mCapacity;
      if ( fabs(v10 - mAnimValue) >= 0.000001 )
        this->mAnimValue = (float)((float)(v10 - mAnimValue) * 0.1) + mAnimValue;
  ...
    SC::cUnitModel::MoveToTimeRelative(this, id: this->mAnimID, time);
```

`src\SC\cUnitModel\UpdateModelAnimations.c:125-156` —— 逐条 `cAnimModel` 求时间（`mV1 + (mV2-mV1)*value`），再夹到 `GetAnimationRange` 给出的片段区间，最后 `MoveToTime`：

```c
      v22 = *(float *)((char *)&v21->mV1 + v4);
      v23 = *(float *)((char *)&v21->mV2 + v4);
      ...
      v26 = (float)((float)(v23 - v22) * v24[2]) + v22;
      ...
        SC::cChoreographyAnimations::GetAnimationRange(this: mpObject, id: v27, start: &a0, finish: &a1);
      ...
      if ( v26 > a1 )
        v35 = a1;
      ...
        SC::cUnitModel::MoveToTime(this, id: v30, time: v35);
```

`all_types.h:71975-71979` —— `cAnimModel` 就是"动画 id + 资源 bin 索引 + 归一值 + 值域"：

```c
struct __cppobj SC::cAnimModel : SC::cBinAnimInfo
{
  float mV1;
  float mV2;
};
```

`all_types.h:71107` —— `struct __cppobj SC::cChoreographyAnimations : EA::RefCountTemplate<int>`（另有 `GetAnimationRange/ActivateAnimSequence/SetAnimTime/Update` 等 14 个函数，见 `src\SC\cChoreographyAnimations\index.md`）。

#### 3.2 建筑/近景网格：CPU 关节蒙皮（`cSimpleAnims`）

`all_types.h:136964-136972`：

```c
struct __cppobj SC::cSimpleAnimJoint
{
  int mBone;
  bool mIsRotation;
  cSPVector3 mRotationAxis;
  float mRotationHalfTheta;
  cSPVector3 mRotationCenter;
  cSPVector3 mTranslationAmount;
};
```

`src\SC\cGraphicsSimpleMesh_AddInstanceVertices.c:15-19` —— 直接对**网格顶点缓冲**与**权重缓冲**加锁并逐顶点算关节变换（`pParams->mAnims` 就是 `cSimpleAnims`）：

```c
void __cdecl SC::cGraphicsSimpleMesh_AddInstanceVertices(
        SC::cAddModel_Params *pParams,
        rw::graphics::VertexBuffer *meshVB,
        rw::graphics::VertexBuffer *weightVB,
        unsigned __int8 unitInstanceInfoSlot)
```

`src\SC\cGraphicsSimpleMesh_AddInstanceVertices.c:76-90`：

```c
    mpObject = pParams->mAnims.mpObject;
    if ( mpObject != nullptr )
    {
      pParams->mAnims.mpObject = nullptr;
      mpObject->Release(this: mpObject);
    }
  ...
  mBoneIndicesIdx = pParams->mBoneIndicesIdx;
  if ( mBoneIndicesIdx < 0 )
    v28 = nullptr;
  else
    v28 = &v34.vertices[meshVB->m_desc->m_elements[mBoneIndicesIdx].offset];
```

调用链：`src\SC\cGraphicsUnitMeshBatcher\AddMeshLODs.c:101` → `cGraphicsMeshBatch::AddModelInstance(..., anims)`（`src\SC\cGraphicsMeshBatch\AddModelInstance.c:15-16`）→ `cGraphicsSimpleMesh::AddModel`（`src\SC\cGraphicsMeshBatch\AddModelInstance.c:148`）。**这条链属于 unit（建筑）网格批处理，不是小人。**

#### 3.3 粒子特效

`src\SC\cAnimatingParticleEffect\index.md:5` —— `?ApplyEffect@cAnimatingParticleEffect@SC@@UAEXMMPAUcComponentStats@Swarm@EA@@@Z 0x650440`，即 `EA::Swarm` 特效组件接口实现；与 agent/小人无任何调用关系（本树无 `cAnimatingParticleEffect` 被 agent 图形代码引用的记录）。

#### 3.4 `cAnimNametable` / `cDrivenKeys`：只剩序列化钩子

`src\cAnimNametable\index.md` 与 `src\cDrivenKeys\index.md` 各 5 个函数，全是 `Fixup/Refix/RegisterArenaReadCallbacks/RegisterArenaWriteCallbacks/Unfix`；唯一调用点在渲染器初始化：

`src\SP\cRenderer\InitArenaSystem.c:55-63`：

```c
  cDrivenKeys::RegisterArenaReadCallbacks();
  cAnimNametable::RegisterArenaReadCallbacks();
  ...
  cDrivenKeys::RegisterArenaWriteCallbacks();
  cAnimNametable::RegisterArenaWriteCallbacks();
```

### 未定 / 反证

- **"小人无关键帧"要精确表述**：小人**用到的部件模型本身**可以带骨骼动画（`cModelInstanceAnimations::ConstructFromResource/SetActive`，`src\SC\cImpostorRenderer\InitImpostorClass.c:391-435`），只是这条动画**属于部件资源**，对同 class 的所有实例**共享同一相位**，不按实例区分。因此正确说法是"**逐实例没有独立动画时间线**"，而不是"小人完全没有骨骼动画"。
- 图集是否**确实每帧**重绘：代码侧证据是 ①`cImpostorRenderer::Update` 每帧 `Animate`；②`RenderOffscreen` 无任何 dirty/间隔判定；③"ImpostorOffscreen" 被注册为 render-script 命令。**但命令的调用频率由 `.pkg` 里的 render script 决定**，本树无法证明是逐帧。**【推理】逐帧重绘**；若某处只在需要时下发该命令，则所有小人会冻结在同一姿态——与 ① 的存在相矛盾，故逐帧最可信。
- `cAnimNametable`/`cDrivenKeys` 的"无消费方"是**grep 阴性**结论：它们的成员函数若被内联进别处，本树看不到。只能表述为"本函数树内没有动画消费代码"。
- 车辆是否同构：`cVehicleAgent` / `cTransportVehicle` 走 `cGraphicsInstancedModelInstance`（`0xD897503` 分支，`src\SC\cGraphicsGame\UpdateTransports.c:370-418`）与 `cGraphicsInstancedImpostor`（`0xD897505`，树 impostor，`:537-586`），**不是** `cGraphicsInstancedSim` 那条链。本次未逐行取证车辆动画，只标注分叉点。**（未定）**

---

## 4. 顶点/像素着色器侧：实例数据到底编码了什么

### 结论

小人精灵的**逐实例**数据只有 5 个量，全部经顶点属性送进着色器：

| 顶点通道 | 内容 | 来源 |
| --- | --- | --- |
| `POSITION` | 世界位置（4 顶点相同） | `mAgentTransforms[i].mTranslation` |
| `NORMAL.x` | `scale × cImpostorType::mScaleFactor` | `ppScales[i]` |
| `NORMAL.y` | **部件号**（bit index） | `modelFlag` 的最低位 |
| `NORMAL.z` | **方位帧号**（0..numAngles-1） | `mCosAngleTable` 二分 |
| `COLOR` | `simInfo`：**R=skin 索引，G=outfit 索引，B=0，A=0** | `GetSimData` 打包 |
| `TEXCOORD.x` | 角点号 0..3 | 顶点序号 |
| `TEXCOORD.y` | 挂接点索引（无则 -1） | `mAttachPoints[部件号]` |

**关于"用实例色 R/G 当调色板 U 坐标"这一既往结论**：本次从 C 代码**证实了实例色的位域含义**——`simInfo = (outfit | (randomBits&0xFF)<<8) << 8`，按 D3DCOLOR 的 `BGRA` 内存序落位后 `R = randomBits&0xFF`（`GetSimPaletteData` 里就叫 `skin`），`G = outfit`。这与"`In.color.x` 查 bodyPaletteTex、`In.color.y` 查 clothesPaletteTex"完全吻合（`x`=skin、`y`=outfit）。

但**像素着色器内的 `FindPalEntry(sourceTex)` 公式（R=aoFactor / G=palV / A=alpha）不在 C 树内**——它只存在于 shader 文本/字节码。本次能给出的、代码侧的补强证据是另一条同构链：**vignette 里的"近景真小人"** 用 `SC::cShaderDataSimPalette` 把调色板索引上传成**归一化 UV**（`index/256 + 1/512`，即 256 项调色板条带的半像素中心），这直接证明了"调色板 = 256 项一维纹理，索引用作 U"的约定。

### 证据

`src\SC\cGraphicsInstancedSim\GetSimData.c:106` —— `simInfo` 打包（**唯一**的实例色来源）：

```c
  *simInfo = (outfit | ((unsigned __int8)randomBits << 8)) << 8;
```

按小端 `D3DCOLOR = 0xAARRGGBB` 展开（**【推理】**，纯算术）：
`byte0(B)=0`、`byte1(G)=outfit`、`byte2(R)=randomBits&0xFF`、`byte3(A)=0`。

`src\SC\cGraphicsInstancedSim\GetSimPaletteData.c:34` —— 同一 `randomBits` 的最低字节被命名为 `skin`：

```c
  *skin = randomBits;
```

`src\SC\cGraphicsInstancedSim\GetSimPaletteData.c:62-70` —— 同一套位域解出 body/head/outfit：

```c
      mBody = v7 % i->mNumBodies + i->mBody;
      mHead = v7 / i->mNumBodies % i->mNumHeads + i->mHead;
      mOutfitPalette = v7 / i->mNumBodies / i->mNumHeads + i->mOutfitPalette;
  ...
      *bodyIndex = mBody;
      *headIndex = mHead;
      *outfit = mOutfitPalette;
```

`src\SC\cGraphicsGame\UpdateTransports.c:481-483` —— **数据视图（data view）模式会覆盖 `simInfo`**，换成调色板颜色（此时小人变成纯色块，不再走 palette 查色）：

```c
                    v64 = _mm_shuffle_epi32(_mm_cvtps_epi32(_mm_mul_ps(*(__m128 *)mRandomID_low, v123)), 198);
                    v65 = _mm_packs_epi32(v64, v64);
                    ppData[v54++] = _mm_cvtsi128_si32(_mm_packus_epi16(v65, v65));
```

`all_types.h:56261-56266` —— 近景小人的调色板常量块（20 字节）：

```c
struct __cppobj SC::cShaderDataSimPalette
{
  unsigned __int8 mVariation;
  unsigned __int8 mPad[3];
  cSPVector4 mPaletteEntries;
};
```

`src\SC\cVignettePool\UpdateVignettePool.c:754-771` —— **索引 → UV 的换算**（256 项、半像素偏移）：

```c
                        v113 = (float)*(int *)(v78 + 96) * 0.00390625;
                        copy.mPaletteEntries.x = (float)((float)*(int *)(v78 + 100) * 0.00390625) + 0.001953125;
                        *(_WORD *)copy.mPad = 0;
                        copy.mPad[2] = 0;
                        copy.mPaletteEntries.y = v113 + 0.001953125;
                        copy.mPaletteEntries.z = 0.0;
                        copy.mPaletteEntries.w = 0.0;
                        copy.mVariation = *(_BYTE *)(v78 + 161) != 0;
                        ...
                        SP::cMaterialInfo::AddShaderData(this: *v112, data: v115);
```

（`0.00390625 = 1/256`，`0.001953125 = 1/512`。）

`src\SC\RegisterShaderData.c:239-244` —— 该块的注册（`0x2C9`，20 字节）：

```c
  SP::RegisterShaderData(
    dataID: 0x2C9u,
    dataSize: 0x14u,
    upload: UploadSimPalette,
    ...
```

`src\SC\cImpostorRenderer\RegisterType.c:60` —— 小人 impostor 的**材质名**（默认值，可被属性 `0x0BD6257D` 覆盖），即既往文档中 `simParticle*` 材质/technique 的代码侧锚点：

```c
  materialId = EA::StdC::FNV1_String8(pData8: "simParticle", nInitialValue: 0x811C9DC5, charCase: kCharCaseLower);
```

`src\SC\cImpostorRenderer\RegisterType.c:61-65` —— 材质与全局缩放：

```c
  SP::GetPropertyAsKeyInstance(list: v5, id: 0xBD6257Du, result: &materialId);
  v8 = SP::MaterialManager();
  particleMaterial = v8->GetMaterial(this: v8, a2: materialId);
  scaleFactor = 1.0;
  SP::GetPropertyAsFloat(list: v5, id: 0xBD6257Fu, result: &scaleFactor);
```

**【数据侧】既往 shader 文本提取**（`docs\re\props-effects-spawners-paths-engine-flow.md:400-405`，非本次 C 取证）：

> `simParticleLightPS` 的真实链路不是把部件纹理直接当漫反射：`FindPalEntry(sourceTex)` 读取 `R=aoFactor`、`G=palV`、`A=alpha`，再以实例 `In.color.x/y` 作为 `bodyPaletteTex` / `clothesPaletteTex` 的 U 坐标查色；服装调色板返回值的 alpha 决定 `lerp(bodyColor, clothesColor, mask)`。

本次代码证据与之一致（R/G 位域 = skin/outfit），但**`FindPalEntry` 的具体公式与两张纹理的名字无法在 C 树内验证**。

### 未定 / 反证

- **实例色 A=0（普通模式）**：`simInfo` 最高字节恒 0。因此小人不透明度**不可能**来自 `In.color.w`；只能来自图集纹理 alpha 或常量（**【推理】**）。这对"复刻 shader"是个硬约束：若照抄"`A=alpha`"到实例色通道，小人会全透明。
- **`NORMAL` 通道被复用为非语义数据**（scale / 部件号 / 方位帧），说明这条 impostor 顶点着色器**不做光照**（法线无意义）。任何"按法线打光"的复刻都会与引擎不符。
- **`mVariation` 只对近景 actor 生效**：精灵链（`RenderParticlesForType`）只推 `0x2CE/0x2C4/0x2CD` 三块，**没有推 `0x2C9`**；`simParticle` 精灵的调色板完全靠实例色 R/G + 材质内建常量。近景 `simModel`/actor 走 `0x2C9`。两条路的调色板入口不同，**不能互相套用**。
- **`cShaderDataImpostorParticles` 的 `mCardData/mCardDepths` 对小人是否被 shader 使用未知**：`InitImpostorClass` 里这两组被清零（`src\SC\cImpostorRenderer\InitImpostorClass.c:543-550` `= kSPZero4_238`），只有 `mTextureTransform` 被写入矩形；真正的 `mCardData` 要到 `RenderOffscreenForModel` 才按外包盒写入。若小人材质不读 `mCardData`，卡片尺寸就只能来自顶点 `NORMAL.x`（scale）+ 材质常量。**【未定】**

---

## 5. 朝向与"两姿态"切换的时序

### 结论

**朝向**：没有任何"小人实体旋转"这一步。agent 的 `cSPTransform::mRotation` 由交通层用"**把局部 +Y 对齐到速度方向（Z-up）**"构造（`AlignYToDirWithZUp`）；渲染层只取这个矩阵的 **`yAxis`** 当"朝向向量"写进 impostor 缓冲；顶点阶段再用它的 `x/y` 分量在 `mCosAngleTable` 里二分出**方位帧号**。因此"面朝行进方向"是**图集方位帧选择**的结果，而不是模型绕 Z 轴旋转。

**两姿态**（idle / walk）切换：**纯瞬时速度阈值**，无频率、无计时器、无迟滞：

- 判据 `|v|² > kSimIdleSpeed2 = 0.01`，即 `|v| > 0.1`；
- 命中 → body 槽 `3*body+1`（walk 部件）；否则 → `3*body+0`（idle 部件）；
- `v` 是"本帧位移/dt"的差分速度，所以**一帧延迟**；
- 走路动作本身的节奏不在这里——它来自 walk 部件的骨骼动画，由 `cImpostorRenderer::Update` 用 `dt × mAnimSpeed` 推进并**整类共享**（`mAnimSpeed` 是部件属性，缺省 1.0；数据侧小人 class 为 6/4 → 身体与头各自不同速率，**【数据侧】**）。

也就是说：**没有任何"速度相关的步频"代码**。若需要按速度改步频，引擎没有做（至少这条链上没有），行走快慢只体现为位移快慢 + 图集帧的同一播放速率。

### 证据

`src\SC\cTransportBase\AgentTransform.c:55-67` —— 行人（基类路径）朝向 = 速度方向：

```c
        v8 = *(float *)(v7 + 72);
        v9 = *(float *)(v7 + 80);
        v10 = 1.0 / fsqrt((float)((float)(v8 * v8) + (float)(*(float *)(v7 + 76) * *(float *)(v7 + 76))) + (float)(v9 * v9)) + 0.0000000099999999;
        v11 = *(float *)(v7 + 76);
        forward.x = v8 * v10;
        forward.y = v11 * v10;
        forward.z = v9 * v10;
        EA::Swarm::AlignYToDirWithZUp(v: &forward, &orient);
        cSPMatrix3::cSPMatrix3(this: &v24, chart: (const SC::cPedestrianChart *)&orient);
        cSPTransform::PostRotateBy(this: transform, rotation: (const cSPMatrix3 *)&v24);
```

`src\SC\cTransportBase\AgentTransform.c:100-104` —— 另一分支用"绕 up 轴转 `AngleBetweenNormalizedVectors(前向, v)`"（等价效果）：

```c
      forward.x = v18;
      forward.y = v19;
      forward.z = v20;
      angle = SP::AngleBetweenNormalizedVectors(a: &kSPForwardVector_338, b: &forward, planeNormal: &kSPUpVector_338);
      cSPTransform::PreRotateAxis(this: transform, axis: &kSPUpVector_338, angle);
```

`src\SC\cTransportDirect\FindAgentTransforms.c:68-81` —— 直达型 agent 同样 `AlignYToDirWithZUp(归一化速度)` 后 `PostRotateBy`：

```c
          x = v11->mVelocity.x;
          v13 = 1.0 / fsqrt((float)((float)((float)(x * x) + (float)(v11->mVelocity.y * v11->mVelocity.y)) + (float)(v11->mVelocity.z * v11->mVelocity.z)) + 0.0000000099999999);
          v14 = v11->mVelocity.y * v13;
          v15 = v11->mVelocity.z * v13;
          v.x = x * v13;
          v.y = v14;
          v.z = v15;
          EA::Swarm::AlignYToDirWithZUp(&v, &orient);
          rotation = (cSPMatrix3)orient;
          cSPTransform::PostRotateBy(this: (cSPTransform *)v8, &rotation);
```

`src\SC\cGraphicsGame\UpdateTransports.c:515-519` —— 只把 `yAxis` 交给渲染：

```c
                  v73 = ppDirections;
                  ppDirections[v66].x = v67->mRotation.yAxis.x;
                  ...
                  v74->z = v67->mRotation.yAxis.z;
```

`src\global\RenderOneParticleBatch.c:57-86` —— **方位帧的运行时选择**：先算 `sinφ+1`（`φ` = 朝向方位角），再用 `mCosAngleTable` 做 3 次手展开二分，`min == numAngles` 时回绕到 0：

```c
      v6 = *pRPBI->mColorData;
      mFacings = pRPBI->mFacings;
      v8 = *pRPBI->mScales * pRPBI->mScaleFactor;
      v9 = (float)(mFacings->y
                 / (float)((float)((float)(mFacings->y * mFacings->y) + (float)(mFacings->x * mFacings->x))
                         + 0.0000099999997))
         + 1.0;
      if ( mFacings->x < 0.0 )
        LODWORD(v9) ^= _mask__NegFloat_;
      max = pRPBI->mNumAngles;
      min = 0;
      if ( v9 <= pRPBI->mCosAngleTable[max >> 1] )
        max >>= 1;
      else
        min = (max >> 1) + 1;
      ...
      v13 = (float)(int)(pRPBI->mNumAngles != min ? min : 0);
```

（注意：**建表用 sin 定符号、cos 定数值，查询却用 `y` 定数值、`x` 定符号**——两侧 sin/cos 角色互换，等价于把 0 号方位定义在 +X 而非 +Y。这是字面事实，不是笔误。）

`src\SC\cGraphicsInstancedSim\GetSimData.c:100-104` —— 姿态槽由速度阈值决定（`+1` 那一位）：

```c
  *modelFlag = (1LL << (head + 9))
             | (1LL << (body
                      + ((float)((float)((float)(velocity.z * velocity.z) + (float)(velocity.y * velocity.y))
                               + (float)(velocity.x * velocity.x)) > kSimIdleSpeed2)
                      + 2 * body));
```

`D:\ea-games\simcity_dev\SimCity.unwrapped.exe.c:170402`：

```c
const float kSimIdleSpeed2 = 0.010000001; // idb
```

`src\SC\cImpostorRenderer\Update.c:45` —— 走路动作的真实时间推进（唯一的时间源，且与实例无关）：

```c
          SP::cModelInstanceAnimations::Animate(this: mpObject, deltaTime: v9->mAnimSpeed * dt);
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:487-492` —— `mAnimSpeed` 来自部件属性，缺省 1.0：

```c
          if ( v100 != 0 )
            v59 = *(_DWORD *)(v100 + 4 * v89);
          else
            v59 = 1065353216;          // 1.0f
          v60 = v87;
          LODWORD(v29->mAnimSpeed) = v59;
```

`src\SC\cImpostorRenderer\InitImpostorClass.c:424-435` —— 每部件只激活**一条**动画序列（`animGroup 0`、`oneShot=false`），没有逐实例变体：

```c
          if ( v97 != 0 )
          {
            v49 = *(_DWORD *)(LODWORD(v86) + v97);
            if ( v49 != 0 )
              SP::cModelInstanceAnimations::SetActive(
                this: v84, id: v49, animGroup: 0, active: true, blendTime: 0.0, oneShot: false);
          }
```

### 未定 / 反证

- **"两姿态翻转"若指'迈左腿/迈右腿'两帧交替，则代码里不存在**：本链只有"idle 部件 / walk 部件"两个**不同部件**的切换（2 个 bit），左右脚交替若有，也只可能发生在 walk 部件的骨骼动画里（图集重绘），**代码不可见**。
- **无迟滞**：`> 0.01` 是硬阈值。理论上速度在 0.1 附近抖动会让 body 部件每帧来回换（视觉上闪烁）。代码里没有任何 `hysteresis`/`smooth`/`cooldown` 字段，这是一个**可复现的引擎行为风险点**（若复刻，建议自行加迟滞，但那就是偏离原版）。
- **方位帧数与"8"**：代码上限 8（`InitImpostorClass.c:173-174`），实际值来自属性表；数据侧小人 class 为 8（**【数据侧】**）。方位帧数是**每 class** 的，不是每实例。
- **`mFacings` 在 `x≈0 且 y≈0` 时退化**：分母加了 `1e-5` 防零，此时 `v9 ≈ 1.0`，会落到表中接近 0 的项——即"速度几乎为零的瞬间朝向仍取上帧 `yAxis`"，不会崩，但会有一次跳变。**【推理】**
- 车辆朝向是否同样用 `yAxis`：`cGraphicsInstancedModelInstance::CreateImmediateInstance(transform)` 直接吃整个变换矩阵（`src\SC\cGraphicsGame\UpdateTransports.c:391-394`），不经过 `ppDirections`。**车辆朝向机制本次未取证（未定）。**

---

## 附录 A：本次钉死的关键常量/上限

| 名称/字面量 | 值 | 出处 |
| --- | --- | --- |
| `kSimIdleSpeed2` | `0.010000001`（比较对象是速度平方和 ⇒ 阈值 ≈0.1 u/s） | 单文件总反编译 `:170402` |
| head 部件起始 bit | `9` | `GetSimData.c:100` |
| body 槽步长 | `3`（`body + 2*body`），walk 占 `+1` | `GetSimData.c:101-104` |
| body 上界守卫 | `<= 3` | `GetSimData.c:91` |
| head 上界守卫 | `<= 0x32`（=50） | `GetSimData.c:91` |
| 部件数上限 | `64`（`fixed_vector<...,64>` / `min(rows,64)`） | `all_types.h:74599`、`InitImpostorClass.c:292-295` |
| 方位帧数 | `[1,8]` | `InitImpostorClass.c:173-176` |
| 图集尺寸 | `512×512`（UV 归一化 1/512） | `InitImpostorClass.c:537-541` |
| 精灵顶点 stride | `36`（V3F+N3F+C4B+T2F） | `BuildVertexBufferForType.c:295` |
| 每批四边形上限 | `2046`（`0x7FE`） | `BuildVertexBufferForType.c:228,312` |
| 调色板项数 | `256`（UV = `idx/256 + 1/512`） | `UpdateVignettePool.c:754-757` |
| 小人材质默认名 | `"simParticle"` | `RegisterType.c:60` |
| impostor class（小人） | `22706EFA`（数据侧；代码里 `0xC36D30D` 是"impostor type"属性 id） | 【数据侧】+ `FillFromProps.c:229` |
| 小人 part 数 | `29` = 9 body + 20 head | 【数据侧】`docs\re\props-effects-spawners-paths-engine-flow.md:364-377` 与 §1 算术互证 |

## 附录 B：等价伪码（**【推理】** 重写，非逐字代码）

```c
// 每帧，每个 crowd sim 实例：
v = (pos_now - pos_prev) / dt;                       // UpdateTransports.c:446-457
flag  = (1ull << (head + 9)) | (1ull << (body*3 + (dot3(v,v) > 0.01f)));   // GetSimData.c:100
info  = (outfit<<8) | (skin<<16);                    // R=skin, G=outfit, A=0
// 顶点阶段，每个置位 bit：
for (part = lowest_set_bit(flag); part < 64; part = next_set_bit(flag)) {
    angle = binsearch_angle( facings.y/(facings.x^2+facings.y^2+1e-5) );   // RenderOneParticleBatch.c:60-86
    emit_quad(pos, color=info, n=(scale, part, angle), uv_x=corner 0..3, uv_y=attach[part]);
}
// 离屏（每帧）：each part, each angle -> DrawBuffersInstanced(instanceCount=mNumAngles)
//                with Animate(dt*mAnimSpeed) 已推进的骨骼姿态
```

## 附录 C：本次未覆盖 / 建议后续取证

1. 车辆（`0xD897503` 模型实例 / `0xD897505` 树型 impostor）的动画与朝向。
2. 建筑 unit-impostor（`cUnitImpostorRenderer` + `cImpostorCell`）的单元格纹理变换与刷新条件。
3. `simParticle` 材质的实际 technique 列表与 `bodyPaletteTex`/`clothesPaletteTex` 绑定（需 shader 容器，不在 C 树）。
4. 属性表 `22706EFA` 的逐行内容（可一次性验证 9/3 布局与第 3 个 body 槽是否为空）。
