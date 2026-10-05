/** @file
 *  @brief src/SC/cImpostorRenderer/InitImpostorClass.c: SC/cImpostorRenderer::InitImpostorClass - decompiled function 0x693490
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?InitImpostorClass@@cImpostorRenderer@@SC@@@@AAE_NPAUcImpostorClass@@2@@IPAVcPropertyList@@SP@@@@@@Z
 *  Address: 0x693490
 */
// mangled: ?InitImpostorClass@cImpostorRenderer@SC@@AAE_NPAUcImpostorClass@2@IPAVcPropertyList@SP@@@Z
// addr: 0x693490
// demangled-sig: char __thiscall SC::cImpostorRenderer::InitImpostorClass(         SC::cImpostorRenderer *this,         SC::cImpostorClass *pClass,         unsigned int classId,         SP::cPropertyList *pProps)
// Incoming xrefs for ?InitImpostorClass@cImpostorRenderer@SC@@AAE_NPAUcImpostorClass@2@IPAVcPropertyList@SP@@@Z (0x693490): None
// Outgoing xrefs for ?InitImpostorClass@cImpostorRenderer@SC@@AAE_NPAUcImpostorClass@2@IPAVcPropertyList@SP@@@Z (0x693490): None
// --- Function: ?InitImpostorClass@cImpostorRenderer@SC@@AAE_NPAUcImpostorClass@2@IPAVcPropertyList@SP@@@Z (0x693490) ---
// offset: RVA 0x293490 EA 0x693490
char __thiscall SC::cImpostorRenderer::InitImpostorClass(
        SC::cImpostorRenderer *this,
        SC::cImpostorClass *pClass,
        unsigned int classId,
        SP::cPropertyList *pProps)
{
  SP::cPropertyList *mpObject; // edi
  rw::graphics::VertexBuffer *VertexBuffer; // eax
  unsigned __int8 *v6; // eax
  unsigned int stride; // edx
  unsigned __int8 *v8; // ecx
  int i; // eax
  int mNumAngles; // ecx
  float v11; // xmm1_4
  float v12; // xmm0_4
  double v13; // xmm0_8
  int v14; // ecx
  int v15; // eax
  float v16; // xmm0_4
  float *v17; // eax
  int v18; // ecx
  bool v19; // cc
  int v20; // esi
  float *mCosAngleTable; // edi
  float v22; // xmm0_4
  float v23; // xmm0_4
  float v24; // xmm0_4
  int v25; // eax
  signed int v26; // esi
  int v27; // eax
  float *p_z; // esi
  SC::cImpostorClassPart *v29; // edi
  float x; // xmm0_4
  float v31; // xmm2_4
  int v32; // ecx
  float v33; // xmm1_4
  float v34; // xmm0_4
  float v35; // xmm0_4
  float v36; // xmm0_4
  float v37; // xmm0_4
  float v38; // xmm0_4
  int v39; // eax
  SP::cModelInstance *v40; // ecx
  int *p_mRefCount; // eax
  EA::Variant *v42; // eax
  SP::cModelInstanceAnimations *v43; // eax
  SP::cResourceBase *Resource; // eax
  SP::cModelInstanceAnimations *v45; // ecx
  int mRefCount; // eax
  SP::cModelInstance *v47; // ecx
  int v48; // edx
  unsigned int v49; // eax
  float v50; // xmm0_4
  EA::Variant *v51; // eax
  int (__thiscall *v52)(EA::Variant *, _DWORD); // edx
  SP::cModelInstance *v53; // eax
  SP::cModelInstance *v54; // ecx
  int v55; // eax
  SP::cModelInstanceAnimations *v56; // ecx
  SP::cModelInstanceAnimations *v57; // eax
  int v58; // eax
  int v59; // xmm0_4
  int v60; // eax
  int v61; // edx
  EA::Variant *v62; // ecx
  float v63; // xmm0_4
  double v64; // st7
  float *v65; // eax
  SP::cModelInstance *v66; // edx
  rw::graphics::Mesh **mpBegin; // eax
  const rw::graphics::VertexDescriptor *mAngleVertexDescriptor; // edx
  const char *v69; // eax
  SP::cRectAllocator::cNode *v70; // edi
  int mLeft; // ecx
  int v72; // eax
  float v73; // xmm3_4
  float v74; // xmm1_4
  bool v75; // zf
  float v76; // xmm0_4
  float v77; // xmm2_4
  float v78; // xmm3_4
  SP::cModelInstanceAnimations *v79; // ecx
  int v80; // eax
  char v82; // [esp+141Dh] [ebp-F9h]
  EA::Variant *v83; // [esp+141Eh] [ebp-F8h] BYREF
  SP::cModelInstanceAnimations *v84; // [esp+1422h] [ebp-F4h]
  SP::cModelInstance *ppModel; // [esp+1426h] [ebp-F0h] BYREF
  float v86; // [esp+142Ah] [ebp-ECh]
  int v87; // [esp+142Eh] [ebp-E8h]
  int v88; // [esp+1432h] [ebp-E4h] BYREF
  int v89; // [esp+1436h] [ebp-E0h]
  float v90; // [esp+143Ah] [ebp-DCh]
  float v91; // [esp+143Eh] [ebp-D8h]
  SC::cImpostorRenderer *v92; // [esp+1442h] [ebp-D4h]
  int v93; // [esp+1446h] [ebp-D0h]
  unsigned int v94; // [esp+144Ah] [ebp-CCh]
  int v95; // [esp+144Eh] [ebp-C8h] BYREF
  int v96; // [esp+1452h] [ebp-C4h] BYREF
  int v97; // [esp+1456h] [ebp-C0h] BYREF
  rw::graphics::VertexBuffer::Locked locked; // [esp+145Ah] [ebp-BCh] BYREF
  int mBottom; // [esp+1466h] [ebp-B0h]
  int v100; // [esp+146Ah] [ebp-ACh] BYREF
  int v101; // [esp+146Eh] [ebp-A8h] BYREF
  int v102; // [esp+1472h] [ebp-A4h] BYREF
  int v103; // [esp+1476h] [ebp-A0h] BYREF
  int v104; // [esp+147Ah] [ebp-9Ch] BYREF
  const rw::graphics::VertexDescriptor *descs[2]; // [esp+147Eh] [ebp-98h] BYREF
  SP::cTablePropInfo tablePropInfo; // [esp+1486h] [ebp-90h] BYREF
  int v107; // [esp+1492h] [ebp-84h]
  int v108; // [esp+1496h] [ebp-80h]
  int *v109; // [esp+149Ah] [ebp-7Ch]
  int v110; // [esp+149Eh] [ebp-78h]
  int v111; // [esp+14A2h] [ebp-74h]
  int *v112; // [esp+14A6h] [ebp-70h]
  int v113; // [esp+14AAh] [ebp-6Ch]
  int v114; // [esp+14AEh] [ebp-68h]
  int *v115; // [esp+14B2h] [ebp-64h]
  int v116; // [esp+14B6h] [ebp-60h]
  int v117; // [esp+14BAh] [ebp-5Ch]
  int *v118; // [esp+14BEh] [ebp-58h]
  int v119; // [esp+14C2h] [ebp-54h]
  int v120; // [esp+14C6h] [ebp-50h]
  int *v121; // [esp+14CAh] [ebp-4Ch]
  int v122; // [esp+14CEh] [ebp-48h]
  int v123; // [esp+14D2h] [ebp-44h]
  int *v124; // [esp+14D6h] [ebp-40h]
  int v125; // [esp+14DAh] [ebp-3Ch]
  int v126; // [esp+14DEh] [ebp-38h]
  int *v127; // [esp+14E2h] [ebp-34h]
  int v128; // [esp+14E6h] [ebp-30h]
  int v129; // [esp+14EAh] [ebp-2Ch]
  int *v130; // [esp+14EEh] [ebp-28h]
  int v131; // [esp+14F2h] [ebp-24h]
  int v132; // [esp+14F6h] [ebp-20h]
  int v133; // [esp+14FAh] [ebp-1Ch]
  SP::cModelInstanceAnimations *v134; // [esp+1502h] [ebp-14h]
  int v135; // [esp+1512h] [ebp-4h]

  v92 = this;
  SC::cImpostorRenderer::ShutdownImpostorClass(this, pClass);
  mpObject = pClass->mConfig.mpObject;
  if ( pProps != mpObject )
  {
    if ( pProps != nullptr )
      pProps->AddRef(this: pProps);
    pClass->mConfig.mpObject = pProps;
    if ( mpObject != nullptr )
      mpObject->Release(this: mpObject);
  }
  pClass->mId = classId;
  memset(a1: &pClass->mShaderAngles, Val: 0, Size: sizeof(pClass->mShaderAngles));
  memset(a1: &pClass->mParticleData, Val: 0, Size: sizeof(pClass->mParticleData));
  memset(a1: &pClass->mAttachPoints, Val: 0, Size: sizeof(pClass->mAttachPoints));
  memset(a1: &pClass->mShadowParticleData, Val: 0, Size: sizeof(pClass->mShadowParticleData));
  memset(a1: &pClass->mShadowAttachPoints, Val: 0, Size: sizeof(pClass->mShadowAttachPoints));
  v82 = 1;
  if ( pProps != nullptr && pProps->GetProperty_2(this: pProps, a2: 198583670, a3: &v83) != nullptr && v83->mTypeId == 9 )
    pClass->mNumAngles = *EA::Variant::operator<int> int &(this: v83);
  if ( pClass->mNumAngles > 8 )
    pClass->mNumAngles = 8;
  if ( pClass->mNumAngles < 1 )
    pClass->mNumAngles = 1;
  pClass->mRenderOffscreenShadows = false;
  if ( pProps != nullptr && pProps->GetProperty_2(this: pProps, a2: 232654088, a3: &v83) != nullptr && v83->mTypeId == 1 )
    pClass->mRenderOffscreenShadows = *EA::Variant::operator<bool> bool &(this: v83);
  VertexBuffer = SP::CreateVertexBuffer(vbdesc: v92->mAngleVertexDescriptor, numVertices: pClass->mNumAngles, type: 8u);
  pClass->mAngleVB = VertexBuffer;
  locked.vertices = nullptr;
  locked.stride = VertexBuffer->m_stride;
  locked.lockFlags = 2;
  v6 = (unsigned __int8 *)rw::graphics::VertexBuffer::D3D9Lock(
                            this: VertexBuffer,
                            flags: 2u,
                            offset: VertexBuffer->m_stride * VertexBuffer->m_base,
                            size: VertexBuffer->m_numVertices * VertexBuffer->m_stride);
  stride = locked.stride;
  locked.vertices = v6;
  v8 = v6;
  for ( i = 0; i < pClass->mNumAngles; v8 += stride )
  {
    *v8 = i;
    v8[1] = i;
    v8[2] = i;
    v8[3] = i++;
  }
  rw::graphics::VertexBuffer::Unlock(this: pClass->mAngleVB, &locked);
  mNumAngles = pClass->mNumAngles;
  v11 = (float)(kSPPi_238 * 2.0) / (float)mNumAngles;
  v86 = v11;
  pClass->mShaderAngles.mNumRegisters = (mNumAngles + 1) / 2;
  v87 = 0;
  if ( mNumAngles > 0 )
  {
    do
    {
      *(float *)&v83 = (float)v87 * v11;
      v12 = *(double *)__libm_sse2_sin().m128_u64;
      v90 = v12;
      v13 = *(double *)__libm_sse2_cos().m128_u64;
      v14 = v87;
      v15 = v87 / 2;
      v16 = v13;
      if ( (v87 & 1) != 0 )
      {
        v17 = (float *)(&pClass->mId + 4 * v15);
        v17[1106] = v90;
        v17[1107] = v16;
      }
      else
      {
        pClass->mShaderAngles.mAngles[v15].x = v90;
        pClass->mShaderAngles.mAngles[v15].y = v16;
      }
      v11 = v86;
      v18 = v14 + 1;
      v19 = v18 < pClass->mNumAngles;
      v87 = v18;
    }
    while ( v19 );
  }
  v20 = 0;
  if ( pClass->mNumAngles > 0 )
  {
    mCosAngleTable = pClass->mCosAngleTable;
    while ( 1 )
    {
      *(float *)&v83 = (float)((float)v20 + 0.5) * v11;
      v22 = *(double *)__libm_sse2_sin().m128_u64;
      v91 = v22;
      v23 = *(double *)__libm_sse2_cos().m128_u64;
      v24 = v23 + 1.0;
      if ( v91 > 0.0 )
        LODWORD(v24) ^= _mask__NegFloat_;
      *mCosAngleTable = v24;
      ++v20;
      ++mCosAngleTable;
      if ( v20 >= pClass->mNumAngles )
        break;
      v11 = v86;
    }
  }
  pClass->mCosAngleTable[pClass->mNumAngles] = 3.4028235e38;
  v109 = &v88;
  v112 = &v97;
  tablePropInfo.mData = &v104;
  v115 = &v100;
  v114 = -2147483635;
  v123 = -2147483635;
  v111 = -2147483616;
  v126 = -2147483616;
  v117 = 9;
  v120 = 9;
  v124 = &v102;
  v127 = &v95;
  v121 = &v103;
  tablePropInfo.mPropID = 198583671;
  tablePropInfo.mTypeID = 32;
  v107 = 198583672;
  v108 = 57;
  v110 = 198583673;
  v113 = 198583681;
  v116 = 198583674;
  v118 = &v96;
  v119 = 198583675;
  v122 = 198583676;
  v125 = 198583678;
  v128 = 198583680;
  v129 = -2147483630;
  v130 = &v101;
  v131 = 0;
  v132 = 0;
  v133 = 0;
  *(float *)&v25 = COERCE_FLOAT(SP::GetPropertiesAsTable(props: pProps, &tablePropInfo));
  v26 = v25;
  v90 = *(float *)&v25;
  if ( v25 >= 0 )
  {
    if ( v25 <= 64 )
      goto LABEL_36;
    v26 = 64;
  }
  else
  {
    *(float *)&v26 = 0.0;
  }
  v90 = *(float *)&v26;
LABEL_36:
  pClass->mParticleData.mNumParticleTypes = v26;
  pClass->mShadowParticleData.mNumParticleTypes = v26;
  pClass->mAggregateBox.mMin.x = 3.4028235e38;
  pClass->mAggregateBox.mMin.y = 3.4028235e38;
  pClass->mAggregateBox.mMin.z = 3.4028235e38;
  pClass->mAggregateBox.mMax.x = -3.4028235e38;
  pClass->mAggregateBox.mMax.y = -3.4028235e38;
  pClass->mAggregateBox.mMax.z = -3.4028235e38;
  eastl::vector<SC::cImpostorClassPart,eastl::fixed_vector_allocator<64,64,4,0,0,eastl::allocator>>::resize(
    this: &pClass->mParts,
    n: v26);
  v89 = 0;
  if ( v26 > 0 )
  {
    v27 = v88;
    v93 = 0;
    v87 = 0;
    v86 = 0.0;
    v94 = 0;
    p_z = &pClass->mParticleData.mParticles[0].mTextureTransform.z;
    do
    {
      v29 = &pClass->mParts.mpBegin[v94 / 0x40];
      x = pClass->mAggregateBox.mMin.x;
      v31 = pClass->mAggregateBox.mMax.x;
      v32 = v87;
      if ( x <= v31 )
      {
        v33 = *(float *)(v87 + v27);
        if ( x > v33 )
          pClass->mAggregateBox.mMin.x = v33;
        v34 = *(float *)(v32 + v27 + 12);
        if ( v34 > v31 )
          pClass->mAggregateBox.mMax.x = v34;
        v35 = *(float *)(v32 + v27 + 4);
        if ( pClass->mAggregateBox.mMin.y > v35 )
          pClass->mAggregateBox.mMin.y = v35;
        v36 = *(float *)(v32 + v27 + 16);
        if ( v36 > pClass->mAggregateBox.mMax.y )
          pClass->mAggregateBox.mMax.y = v36;
        v37 = *(float *)(v32 + v27 + 8);
        if ( pClass->mAggregateBox.mMin.z > v37 )
          pClass->mAggregateBox.mMin.z = v37;
        v38 = *(float *)(v32 + v27 + 20);
        if ( v38 > pClass->mAggregateBox.mMax.z )
          pClass->mAggregateBox.mMax.z = v38;
      }
      else
      {
        pClass->mAggregateBox.mMin.x = *(float *)(v87 + v27);
        pClass->mAggregateBox.mMin.y = *(float *)(v32 + v27 + 4);
        pClass->mAggregateBox.mMin.z = *(float *)(v32 + v27 + 8);
        pClass->mAggregateBox.mMax = *(cSPVector3 *)(v32 + v27 + 12);
      }
      v83 = *(EA::Variant **)(v96 + 4 * v89);
      if ( (int)v83 <= 0 || *(int *)(v103 + 4 * v89) <= 0 )
        goto LABEL_109;
      v39 = SP::cRectAllocator::AddRect(
              this: &v92->mAllocator,
              w: (_DWORD)v83 * pClass->mNumAngles,
              h: *(_DWORD *)(v103 + 4 * v89));
      v29->mRectID = v39;
      if ( v39 >= 0 )
      {
        ppModel = nullptr;
        v135 = 0;
        if ( SP::CreateModelInstance(
               instanceID: *(_DWORD *)(LODWORD(v86) + v104),
               groupID: *(_DWORD *)(LODWORD(v86) + v104 + 8),
               &ppModel,
               flags: 2) != 0 )
        {
          *(float *)&v42 = COERCE_FLOAT(operator new[](size: 0xA0u, name: nullptr, flags: 0));
          v83 = v42;
          LOBYTE(v135) = 1;
          if ( *(float *)&v42 == 0.0 )
          {
            v43 = nullptr;
            v84 = nullptr;
          }
          else
          {
            v43 = SP::cModelInstanceAnimations::cModelInstanceAnimations(this: (SP::cModelInstanceAnimations *)v42);
            v84 = v43;
          }
          v134 = v43;
          if ( v43 != nullptr )
            ++v43->mRefCount;
          LOBYTE(v135) = 2;
          SP::cModelInstanceAnimations::Init(this: v84, bIgnoreAnimations: v97 == 0, bForceMultiblend: true);
          Resource = SP::cModelInstance::GetResource(this: ppModel);
          if ( SP::cModelInstanceAnimations::ConstructFromResource(this: v84, pRsrc: Resource) == 0 )
          {
            v45 = v84;
            LOBYTE(v135) = 0;
            if ( v84 != nullptr )
            {
              mRefCount = v84->mRefCount;
              v84->mRefCount = mRefCount - 1;
              if ( mRefCount == 1 )
              {
                v45->mRefCount = 1;
                ((void (__thiscall *)(SP::cModelInstanceAnimations *, int))v45->dtr_RefCountTemplate<int>)(
                  a1: v45,
                  a2: 1);
              }
            }
            v47 = ppModel;
            v135 = -1;
            if ( ppModel != nullptr )
            {
              v48 = ppModel->mRefCount;
              ppModel->mRefCount = v48 - 1;
              if ( v48 == 1 )
              {
                v47->mRefCount = 1;
                ((void (__thiscall *)(SP::cModelInstance *, int))v47->dtr_RefCountTemplate<int>)(a1: v47, a2: 1);
              }
            }
            goto LABEL_108;
          }
          SP::cModelInstance::ConstructSharedAnimationData(this: ppModel, anims: v84);
          if ( v97 != 0 )
          {
            v49 = *(_DWORD *)(LODWORD(v86) + v97);
            if ( v49 != 0 )
              SP::cModelInstanceAnimations::SetActive(
                this: v84,
                id: v49,
                animGroup: 0,
                active: true,
                blendTime: 0.0,
                oneShot: false);
          }
          if ( v102 != 0 )
            v50 = *(float *)(v102 + 4 * v89);
          else
            v50 = 0.0;
          v91 = v50;
          *(float *)&v83 = 0.0;
          if ( v95 != 0 && *(_DWORD *)(LODWORD(v86) + v95) != 0 )
          {
            *(float *)&v51 = COERCE_FLOAT(SP::MaterialManager());
            v52 = *(int (__thiscall **)(EA::Variant *, _DWORD))(v51->mInt32 + 48);
            v83 = v51;
            *(float *)&v83 = COERCE_FLOAT(v52(a1: v51, a2: *(_DWORD *)(LODWORD(v86) + v95)));
          }
          v53 = ppModel;
          v54 = v29->mModel.mpObject;
          if ( ppModel != v29->mModel.mpObject )
          {
            if ( ppModel != nullptr )
              ++ppModel->mRefCount;
            v29->mModel.mpObject = v53;
            if ( v54 != nullptr )
            {
              v55 = v54->mRefCount;
              v54->mRefCount = v55 - 1;
              if ( v55 == 1 )
              {
                v54->mRefCount = 1;
                ((void (__thiscall *)(SP::cModelInstance *, int))v54->dtr_RefCountTemplate<int>)(a1: v54, a2: 1);
              }
            }
          }
          v56 = v29->mAnimation.mpObject;
          v57 = v84;
          if ( v84 != v56 )
          {
            if ( v84 != nullptr )
              ++v84->mRefCount;
            v29->mAnimation.mpObject = v57;
            if ( v56 != nullptr )
            {
              v58 = v56->mRefCount;
              v56->mRefCount = v58 - 1;
              if ( v58 == 1 )
              {
                v56->mRefCount = 1;
                ((void (__thiscall *)(SP::cModelInstanceAnimations *, int))v56->dtr_RefCountTemplate<int>)(
                  a1: v56,
                  a2: 1);
              }
            }
          }
          if ( v100 != 0 )
            v59 = *(_DWORD *)(v100 + 4 * v89);
          else
            v59 = 1065353216;
          v60 = v87;
          LODWORD(v29->mAnimSpeed) = v59;
          v61 = v88;
          v62 = v83;
          v63 = v91;
          v29->mBox.mMax.x = *(float *)(v60 + v88 + 12);
          v64 = *(float *)(v60 + v61 + 16);
          v65 = (float *)(v61 + v60);
          v29->mBox.mMax.y = v64;
          v29->mBox.mMax.z = v65[5];
          v29->mBox.mMin.x = *v65;
          v29->mBox.mMin.y = v65[1];
          v29->mBox.mMin.z = v65[2];
          v29->mZBias = v63;
          v29->mOffscreenMaterial = (SP::cMaterial *)v62;
          v29->mChildBoneIdx = -1;
          v29->mInstancingDescriptor = nullptr;
          v66 = ppModel;
          mpBegin = ppModel->mMeshes.mpBegin;
          if ( mpBegin != ppModel->mMeshes.mpEnd )
          {
            mAngleVertexDescriptor = v92->mAngleVertexDescriptor;
            descs[0] = (*mpBegin)->m_vertexBuffer[0]->m_desc;
            descs[1] = mAngleVertexDescriptor;
            v29->mInstancingDescriptor = SP::CombineVertexDescriptorsAsStreams(descs, numDescs: 2);
            v66 = ppModel;
          }
          if ( v101 != 0 )
          {
            v69 = *(const char **)(v93 + v101);
            if ( v69 != nullptr )
            {
              v29->mChildBoneIdx = `anonymous namespace'::FindBoneIndex(pAnims: v84, boneName: v69);
              v66 = ppModel;
            }
          }
          v70 = &v92->mAllocator.mNodes.mpBegin[v29->mRectID];
          LOBYTE(v135) = 0;
          mLeft = v70->mRect.mLeft;
          locked.stride = v70->mRect.mTop;
          mBottom = v70->mRect.mBottom;
          v72 = *(_DWORD *)(v96 + 4 * v89);
          v73 = (float)mBottom;
          v74 = (float)(int)locked.stride * 0.001953125;
          *(p_z - 1) = v74;
          v75 = v84 == nullptr;
          v76 = (float)mLeft * 0.001953125;
          *(p_z - 2) = v76;
          v77 = (float)((float)(mLeft + v72) * 0.001953125) - v76;
          *p_z = v77;
          v78 = (float)(v73 * 0.001953125) - v74;
          p_z[1] = v78;
          *(cSPVector4 *)(p_z + 2) = kSPZero4_238;
          *(cSPVector4 *)(p_z + 6) = kSPZero4_238;
          p_z[896] = v76;
          p_z[897] = v74;
          p_z[898] = v77;
          p_z[899] = v78;
          *((cSPVector4 *)p_z + 225) = kSPZero4_238;
          *((cSPVector4 *)p_z + 226) = kSPZero4_238;
          if ( !v75 )
          {
            v79 = v84;
            v80 = v84->mRefCount;
            v84->mRefCount = v80 - 1;
            if ( v80 == 1 )
            {
              v79->mRefCount = 1;
              ((void (__thiscall *)(SP::cModelInstanceAnimations *, int))v79->dtr_RefCountTemplate<int>)(a1: v79, a2: 1);
            }
            v66 = ppModel;
          }
          v135 = -1;
          if ( v66 == nullptr )
            goto LABEL_108;
          v40 = v66;
          p_mRefCount = &v66->mRefCount;
          goto LABEL_106;
        }
        v40 = ppModel;
        v135 = -1;
        if ( ppModel != nullptr )
        {
          p_mRefCount = &ppModel->mRefCount;
LABEL_106:
          v75 = (*p_mRefCount)-- == 1;
          if ( v75 )
          {
            *p_mRefCount = 1;
            ((void (__thiscall *)(SP::cModelInstance *, int))v40->dtr_RefCountTemplate<int>)(a1: v40, a2: 1);
          }
        }
      }
      else
      {
        v82 = 0;
      }
LABEL_108:
      v27 = v88;
LABEL_109:
      v94 += 64;
      LODWORD(v86) += 12;
      v87 += 24;
      v93 += 16;
      p_z += 12;
      ++v89;
    }
    while ( v89 < SLODWORD(v90) );
  }
  return v82;
}
// --- End Function: ?InitImpostorClass@cImpostorRenderer@SC@@AAE_NPAUcImpostorClass@2@IPAVcPropertyList@SP@@@Z (0x693490) ---
