/** @file
 *  @brief src/SC/cImpostorRenderer/RenderOffscreenForModel.c: SC/cImpostorRenderer::RenderOffscreenForModel - decompiled function 0x691920
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?RenderOffscreenForModel@@cImpostorRenderer@@SC@@@@AAEX_NPAUcRenderStats@@SP@@@@PAUcImpostorClass@@2@@IABUcSPTransform@@@@@@Z
 *  Address: 0x691920
 */
// mangled: ?RenderOffscreenForModel@cImpostorRenderer@SC@@AAEX_NPAUcRenderStats@SP@@PAUcImpostorClass@2@IABUcSPTransform@@@Z
// addr: 0x691920
// demangled-sig: void __thiscall SC::cImpostorRenderer::RenderOffscreenForModel(         SC::cImpostorRenderer *this,         bool isShadow,         SP::cRenderStats *stats,         SC::cImpostorClass *pClass,         unsigned int aggModel,
// Incoming xrefs for ?RenderOffscreenForModel@cImpostorRenderer@SC@@AAEX_NPAUcRenderStats@SP@@PAUcImpostorClass@2@IABUcSPTransform@@@Z (0x691920): None
// Outgoing xrefs for ?RenderOffscreenForModel@cImpostorRenderer@SC@@AAEX_NPAUcRenderStats@SP@@PAUcImpostorClass@2@IABUcSPTransform@@@Z (0x691920): None
// --- Function: ?RenderOffscreenForModel@cImpostorRenderer@SC@@AAEX_NPAUcRenderStats@SP@@PAUcImpostorClass@2@IABUcSPTransform@@@Z (0x691920) ---
// offset: RVA 0x291920 EA 0x691920
void __thiscall SC::cImpostorRenderer::RenderOffscreenForModel(
        SC::cImpostorRenderer *this,
        bool isShadow,
        SP::cRenderStats *stats,
        SC::cImpostorClass *pClass,
        unsigned int aggModel,
        const cSPTransform *camXform)
{
  SC::cImpostorRenderer *v6; // esi
  _DWORD *v7; // eax
  SC::cImpostorClass *v8; // edi
  SC::cImpostorClassPart *v9; // ebx
  bool v10; // zf
  SP::cMaterial *mOffscreenMaterial; // eax
  float v12; // xmm1_4
  float v13; // xmm0_4
  float v14; // xmm2_4
  float v15; // xmm3_4
  float v16; // xmm5_4
  float v17; // xmm7_4
  float v18; // xmm0_4
  double v19; // xmm0_8
  float z; // xmm7_4
  float x; // xmm5_4
  const cSPTransform *v22; // ecx
  float v23; // xmm6_4
  float v24; // xmm7_4
  float v25; // xmm0_4
  float v26; // xmm4_4
  float v27; // xmm5_4
  float v28; // xmm6_4
  SC::cShaderDataImpostorParticles *p_mShadowParticleData; // ecx
  unsigned int v30; // eax
  float mZBias; // xmm0_4
  float v32; // xmm3_4
  float v33; // xmm4_4
  float v34; // xmm5_4
  float v35; // xmm1_4
  float v36; // xmm6_4
  float v37; // xmm3_4
  float v38; // xmm3_4
  float v39; // xmm1_4
  float w; // xmm5_4
  float v41; // xmm4_4
  int v42; // ebx
  bool v43; // cc
  float *p_z; // edi
  int *v45; // esi
  float v46; // xmm0_4
  float v47; // xmm0_4
  float v48; // xmm2_4
  float v49; // xmm6_4
  float v50; // xmm5_4
  float v51; // xmm1_4
  float v52; // xmm4_4
  float v53; // xmm3_4
  float v54; // xmm1_4
  float v55; // xmm2_4
  float v56; // xmm3_4
  float v57; // xmm0_4
  SP::cModelInstance *mpObject; // esi
  rw::graphics::Mesh **mpBegin; // edx
  rw::graphics::Mesh *v60; // ebx
  rw::graphics::IndexBuffer *m_indexBuffer; // ecx
  int m_numIndices; // eax
  SP::cMaterial *data; // [esp+F3Ch] [ebp-128h]
  char *dataa; // [esp+F3Ch] [ebp-128h]
  rw::graphics::IndexBuffer *datab; // [esp+F3Ch] [ebp-128h]
  int v67; // [esp+F40h] [ebp-124h]
  float v68; // [esp+F44h] [ebp-120h]
  rw::graphics::VertexBuffer *v69; // [esp+F44h] [ebp-120h]
  int v70; // [esp+F4Ch] [ebp-118h]
  float v71; // [esp+F54h] [ebp-110h]
  float v72; // [esp+F58h] [ebp-10Ch]
  float v73; // [esp+F5Ch] [ebp-108h]
  float v74; // [esp+F60h] [ebp-104h]
  float v75; // [esp+F64h] [ebp-100h]
  float v76; // [esp+F68h] [ebp-FCh]
  float v77; // [esp+F6Ch] [ebp-F8h]
  float v78; // [esp+F6Ch] [ebp-F8h]
  EA::Swarm::cTransform v79; // [esp+F70h] [ebp-F4h] BYREF
  float v80; // [esp+FA8h] [ebp-BCh]
  float y; // [esp+FACh] [ebp-B8h]
  float v82; // [esp+FB0h] [ebp-B4h]
  float v83; // [esp+FB4h] [ebp-B0h]
  SC::cImpostorClassPart *v84; // [esp+FB8h] [ebp-ACh]
  cSPBoundingBox out; // [esp+FBCh] [ebp-A8h] BYREF
  void *p_mAtlasInfo; // [esp+FD4h] [ebp-90h]
  cSPBoundingBox v87; // [esp+FD8h] [ebp-8Ch] BYREF
  const char *v88; // [esp+FF0h] [ebp-74h]
  SP::cDispatchState v89; // [esp+FF4h] [ebp-70h] BYREF
  int v90; // [esp+1060h] [ebp-4h]

  v6 = this;
  v7 = SP::ShaderData(id: 0x201u);
  if ( v7 != nullptr )
    v70 = v7[1];
  else
    v70 = 0;
  v8 = pClass;
  v9 = &pClass->mParts.mpBegin[aggModel];
  v10 = v9->mModel.mpObject == nullptr;
  v84 = v9;
  if ( v10 )
    return;
  mOffscreenMaterial = v9->mOffscreenMaterial;
  data = mOffscreenMaterial;
  if ( mOffscreenMaterial == nullptr || mOffscreenMaterial->mRTMap[v70].mNumPasses == 0 )
    return;
  SP::PushShaderData();
  v88 = "cImpostorRenderer::RenderOffscreenForModel";
  v90 = 0;
  SP::SetShaderData(type: 0x258u, data, dataChanged: false);
  v12 = 3.4028235e38;
  v13 = -3.4028235e38;
  v14 = 3.4028235e38;
  v15 = 3.4028235e38;
  v16 = -3.4028235e38;
  v17 = -3.4028235e38;
  v71 = 3.4028235e38;
  v72 = 3.4028235e38;
  v73 = 3.4028235e38;
  v74 = -3.4028235e38;
  v75 = -3.4028235e38;
  v76 = -3.4028235e38;
  dataa = nullptr;
  if ( pClass->mNumAngles <= 0 )
    goto LABEL_25;
  do
  {
    cSPTransform::cSPTransform(this: &v79, __that: (const EA::Swarm::cTransform *)camXform);
    v18 = *(double *)__libm_sse2_sin().m128_u64;
    v77 = v18;
    v19 = *(double *)__libm_sse2_cos().m128_u64;
    z = v79.mRotation.xAxis.z;
    x = v79.mRotation.xAxis.x;
    v79.mFlags |= 2u;
    ++v79.mModificationCount;
    *(float *)&v19 = v19;
    y = v79.mRotation.xAxis.y;
    v79.mRotation.xAxis.x = (float)(*(float *)&v19 * v79.mRotation.xAxis.x) + (float)(v77 * v79.mRotation.yAxis.x);
    v79.mRotation.xAxis.y = (float)(*(float *)&v19 * v79.mRotation.xAxis.y) + (float)(v77 * v79.mRotation.yAxis.y);
    v79.mRotation.xAxis.z = (float)(*(float *)&v19 * v79.mRotation.xAxis.z) + (float)(v77 * v79.mRotation.yAxis.z);
    v82 = z;
    v79.mRotation.yAxis.x = (float)(x * COERCE_FLOAT(LODWORD(v77) ^ _mask__NegFloat_))
                          + (float)(*(float *)&v19 * v79.mRotation.yAxis.x);
    v79.mRotation.yAxis.y = (float)(COERCE_FLOAT(LODWORD(v77) ^ _mask__NegFloat_) * y)
                          + (float)(*(float *)&v19 * v79.mRotation.yAxis.y);
    v79.mRotation.yAxis.z = (float)(COERCE_FLOAT(LODWORD(v77) ^ _mask__NegFloat_) * z)
                          + (float)(*(float *)&v19 * v79.mRotation.yAxis.z);
    out.mMin.x = 3.4028235e38;
    out.mMin.y = 3.4028235e38;
    out.mMin.z = 3.4028235e38;
    out.mMax.x = -3.4028235e38;
    out.mMax.y = -3.4028235e38;
    out.mMax.z = -3.4028235e38;
    AnalyzeBoundingBox(&out, bbox: &v9->mBox, xform: (const cSPTransform *)&v79);
    v87.mMin.x = 3.4028235e38;
    v87.mMin.y = 3.4028235e38;
    v87.mMin.z = 3.4028235e38;
    v87.mMax.x = -3.4028235e38;
    v87.mMax.y = -3.4028235e38;
    v87.mMax.z = -3.4028235e38;
    AnalyzeBoundingBox(out: &v87, bbox: &pClass->mAggregateBox, xform: v22);
    v12 = v71;
    v13 = v74;
    if ( v71 > v74 )
    {
      v12 = out.mMin.x;
      v14 = out.mMin.y;
      v15 = v87.mMin.z;
      v13 = out.mMax.x;
      v16 = out.mMax.y;
      v17 = v87.mMax.z;
      v71 = out.mMin.x;
      v72 = out.mMin.y;
      v73 = v87.mMin.z;
      v74 = out.mMax.x;
      v75 = out.mMax.y;
LABEL_22:
      v76 = v17;
      goto LABEL_23;
    }
    if ( v71 > out.mMin.x )
    {
      v12 = out.mMin.x;
      v71 = out.mMin.x;
    }
    if ( out.mMax.x > v74 )
    {
      v13 = out.mMax.x;
      v74 = out.mMax.x;
    }
    v14 = v72;
    if ( v72 > out.mMin.y )
    {
      v14 = out.mMin.y;
      v72 = out.mMin.y;
    }
    v16 = v75;
    if ( out.mMax.y > v75 )
    {
      v16 = out.mMax.y;
      v75 = out.mMax.y;
    }
    v15 = v73;
    if ( v73 > v87.mMin.z )
    {
      v15 = v87.mMin.z;
      v73 = v87.mMin.z;
    }
    v17 = v76;
    if ( v87.mMax.z > v76 )
    {
      v17 = v87.mMax.z;
      goto LABEL_22;
    }
LABEL_23:
    ++dataa;
  }
  while ( (int)dataa < pClass->mNumAngles );
  v6 = this;
LABEL_25:
  v23 = v17;
  LODWORD(v24) = LODWORD(v17) ^ _mask__NegFloat_;
  v25 = v13 - v12;
  v26 = v25;
  v27 = v16 - v14;
  v28 = v23 - v15;
  p_mShadowParticleData = &pClass->mShadowParticleData;
  if ( !isShadow )
    p_mShadowParticleData = &pClass->mParticleData;
  v30 = aggModel;
  p_mShadowParticleData->mParticles[v30].mCardData.x = v12;
  p_mShadowParticleData->mParticles[v30].mCardData.y = v14;
  p_mShadowParticleData->mParticles[v30].mCardData.z = v25;
  p_mShadowParticleData->mParticles[v30].mCardData.w = v27;
  mZBias = v9->mZBias;
  p_mShadowParticleData->mParticles[v30].mCardDepths.w = (float)pClass->mNumAngles;
  p_mShadowParticleData->mParticles[v30].mCardDepths.y = mZBias;
  p_mShadowParticleData->mParticles[v30].mCardDepths.z = v28;
  p_mShadowParticleData->mParticles[v30].mCardDepths.x = v24;
  v32 = 1.0 / v26;
  v33 = 1.0 / v27;
  v34 = 1.0 / v28;
  v35 = (float)-v12 * v32;
  y = (float)((float)((float)-v14 * v33) * 2.0) - 1.0;
  v36 = v9->mOffscreenParams[0];
  v6->mAtlasInfo.mProjScale.x = v32 * 2.0;
  v6->mAtlasInfo.mProjScale.z = v34;
  v6->mAtlasInfo.mProjScale.w = v36;
  v6->mAtlasInfo.mProjScale.y = v33 * 2.0;
  v37 = v9->mOffscreenParams[1];
  v6->mAtlasInfo.mProjOffset.x = (float)(v35 * 2.0) - 1.0;
  v6->mAtlasInfo.mProjOffset.y = y;
  v6->mAtlasInfo.mProjOffset.w = v37;
  v6->mAtlasInfo.mProjOffset.z = (float)-v24 * v34;
  v38 = pClass->mParticleData.mParticles[aggModel].mTextureTransform.x;
  v39 = pClass->mParticleData.mParticles[aggModel].mTextureTransform.z;
  w = pClass->mParticleData.mParticles[aggModel].mTextureTransform.w;
  v41 = pClass->mParticleData.mParticles[aggModel].mTextureTransform.y;
  v42 = 0;
  v43 = pClass->mNumAngles <= 0;
  p_mAtlasInfo = &v6->mAtlasInfo;
  v80 = v38;
  v82 = v39;
  v83 = w;
  if ( !v43 )
  {
    p_z = &this->mAtlasInfo.mScale[0].z;
    v68 = (float)((float)(v41 * 2.0) + w) - 1.0;
    v45 = &this->mAtlasInfo.mTransform[0].yAxis.mV.m128_i32[2];
    while ( 1 )
    {
      *p_z = 1.0;
      p_z[1] = 1.0;
      *(p_z - 2) = v39;
      *(p_z - 1) = w;
      p_z[32] = 0.0;
      p_z[33] = 0.0;
      p_z[31] = v68;
      p_z[30] = (float)((float)(v38 * 2.0) + v39) - 1.0;
      v79.mTranslation = kSPZero3_238.rw::math::fpu::Vector3Template<float>;
      v79.mRotation.zAxis = kSPIdentity3_238.zAxis;
      v46 = *(double *)__libm_sse2_sin().m128_u64;
      v78 = v46;
      v47 = *(double *)__libm_sse2_cos().m128_u64;
      v48 = v47 * kSPIdentity3_238.yAxis.y;
      v49 = COERCE_FLOAT(LODWORD(v78) ^ _mask__NegFloat_) * kSPIdentity3_238.xAxis.y;
      v50 = (float)(COERCE_FLOAT(LODWORD(v78) ^ _mask__NegFloat_) * kSPIdentity3_238.xAxis.x)
          + (float)(v47 * kSPIdentity3_238.yAxis.x);
      v51 = (float)(v47 * kSPIdentity3_238.xAxis.y) + (float)(v78 * kSPIdentity3_238.yAxis.y);
      v52 = (float)(COERCE_FLOAT(LODWORD(v78) ^ _mask__NegFloat_) * kSPIdentity3_238.xAxis.z)
          + (float)(v47 * kSPIdentity3_238.yAxis.z);
      v53 = (float)(v47 * kSPIdentity3_238.xAxis.z) + (float)(v78 * kSPIdentity3_238.yAxis.z);
      *((float *)v45 - 6) = (float)(v47 * kSPIdentity3_238.xAxis.x) + (float)(v78 * kSPIdentity3_238.yAxis.x);
      *((float *)v45 - 5) = v51;
      *((float *)v45 - 4) = v53;
      *((float *)v45 - 2) = v50;
      *((float *)v45 - 1) = v49 + v48;
      *(float *)v45 = v52;
      v54 = v79.mRotation.zAxis.y;
      v55 = v79.mRotation.zAxis.z;
      v56 = v80;
      v45[2] = LODWORD(v79.mRotation.zAxis.x);
      v57 = v79.mTranslation.x;
      *((float *)v45 + 3) = v54;
      v39 = v82;
      *((float *)v45 + 4) = v55;
      *((float *)v45 + 6) = v57;
      *(_QWORD *)(v45 + 7) = *(_QWORD *)&v79.mTranslation.y;
      ++v42;
      p_z += 4;
      v45 += 16;
      v43 = v42 < pClass->mNumAngles;
      v38 = v56 + v39;
      v80 = v38;
      if ( !v43 )
        break;
      w = v83;
    }
    v8 = pClass;
  }
  mpObject = v84->mModel.mpObject;
  memset(a1: &v89, Val: 0, Size: sizeof(v89));
  v79.mTranslation = kSPZero3_238.rw::math::fpu::Vector3Template<float>;
  v79.mScale = 1.0;
  v79.mRotation = kSPIdentity3_238.rw::math::fpu::Matrix33Template<float>;
  v79.mFlags = 0;
  v79.mModificationCount = 0;
  SP::cModelInstance::DispatchTransform(this: mpObject, transform: (const cSPTransform *)&v79, dispatchState: &v89);
  SP::cModelInstance::PrepareDispatch(this: mpObject, dispatchState: &v89);
  if ( mpObject->mMeshes.mpBegin != mpObject->mMeshes.mpEnd )
  {
    SP::PushShaderData();
    mpBegin = mpObject->mMeshes.mpBegin;
    v60 = *mpBegin;
    m_indexBuffer = (*mpBegin)->m_indexBuffer;
    v69 = (*mpBegin)->m_vertexBuffer[0];
    m_numIndices = (*mpBegin)->m_numIndices;
    LOBYTE(v90) = 1;
    datab = m_indexBuffer;
    v67 = m_numIndices;
    if ( m_numIndices == 0 )
      v67 = m_indexBuffer->m_numIndices;
    SP::SetShaderData(type: 0x2A2u, data: p_mAtlasInfo, dataChanged: true);
    if ( SP::SetRenderState(
           state: mpObject->mMaterials.mpBegin->mpObject->mCompiledStates[mpObject->mMaterials.mpBegin->mpObject->mRTMap[v70].mCSIndex],
           matInfo: nullptr,
           rasterCount: 0,
           rasters: nullptr) != 0 )
    {
      SP::cModelInstance::SetMeshSkinningState(this: mpObject, meshIndex: 0, dispatchState: &v89);
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
    }
    LOBYTE(v90) = 0;
    SP::PopShaderData();
  }
  v90 = -1;
  SP::PopShaderData();
}
// --- End Function: ?RenderOffscreenForModel@cImpostorRenderer@SC@@AAEX_NPAUcRenderStats@SP@@PAUcImpostorClass@2@IABUcSPTransform@@@Z (0x691920) ---
