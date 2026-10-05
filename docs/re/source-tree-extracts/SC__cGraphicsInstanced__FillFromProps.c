/** @file
 *  @brief src/SC/cGraphicsInstanced/FillFromProps.c: SC/cGraphicsInstanced::FillFromProps - decompiled function 0x7872f0
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?FillFromProps@@cGraphicsInstanced@@SC@@@@QAE_NPAVcPropertyList@@SP@@@@PAVcIRenderer@@Swarm@@EA@@@@@@Z
 *  Address: 0x7872f0
 */
// mangled: ?FillFromProps@cGraphicsInstanced@SC@@QAE_NPAVcPropertyList@SP@@PAVcIRenderer@Swarm@EA@@@Z
// addr: 0x7872f0
// demangled-sig: char __thiscall SC::cGraphicsInstanced::FillFromProps(         SC::cGraphicsInstanced *this,         SP::cPropertyList *props,         EA::Swarm::cIRenderer *effectRenderer)
// Incoming xrefs for ?FillFromProps@cGraphicsInstanced@SC@@QAE_NPAVcPropertyList@SP@@PAVcIRenderer@Swarm@EA@@@Z (0x7872F0): None
// Outgoing xrefs for ?FillFromProps@cGraphicsInstanced@SC@@QAE_NPAVcPropertyList@SP@@PAVcIRenderer@Swarm@EA@@@Z (0x7872F0): None
// --- Function: ?FillFromProps@cGraphicsInstanced@SC@@QAE_NPAVcPropertyList@SP@@PAVcIRenderer@Swarm@EA@@@Z (0x7872F0) ---
// offset: RVA 0x3872F0 EA 0x7872F0
char __thiscall SC::cGraphicsInstanced::FillFromProps(
        SC::cGraphicsInstanced *this,
        SP::cPropertyList *props,
        EA::Swarm::cIRenderer *effectRenderer)
{
  SP::cPropertyList *mpObject; // edi
  SP::cPropertyList *v5; // ecx
  float v6; // xmm0_4
  float v7; // xmm1_4
  float v8; // xmm2_4
  char result; // al
  __int16 v10; // cx
  unsigned int v11; // eax
  eastl::aligned_buffer<536,8> *v12; // ebx
  SP::cPropertyList *v13; // esi
  signed int PropertiesAsTable; // ebp
  signed int v15; // esi
  unsigned __int8 *v16; // eax
  int v17; // edi
  unsigned __int8 v18; // dl
  unsigned __int8 v19; // cl
  unsigned __int8 v20; // cl
  unsigned __int8 v21; // dl
  unsigned __int8 v22; // cl
  float v23; // xmm0_4
  eastl::aligned_buffer<536,8> *v24; // ebp
  char PropertyAsBoundingBox; // al
  int v26; // edx
  int v27; // eax
  unsigned int *v28; // ecx
  SP::cPropertyList *v29; // esi
  float *v30; // eax
  int v31; // ebx
  float v32; // xmm0_4
  float v33; // xmm1_4
  float v34; // xmm2_4
  float v35; // xmm0_4
  SP::cPropertyList *v36; // [esp-10h] [ebp-11Ch]
  SP::cPropertyList *v37; // [esp-Ch] [ebp-118h]
  SP::cPropertyList *v38; // [esp-Ch] [ebp-118h]
  SP::cPropertyList *v39; // [esp-Ch] [ebp-118h]
  SP::cPropertyList *v40; // [esp-Ch] [ebp-118h]
  float impostorDataViewType; // [esp+10h] [ebp-FCh] BYREF
  bool hasBbox; // [esp+17h] [ebp-F5h]
  unsigned int materialID; // [esp+18h] [ebp-F4h] BYREF
  int numModelKeys; // [esp+1Ch] [ebp-F0h] BYREF
  const EA::ResourceMan::Key *modelKeys; // [esp+20h] [ebp-ECh] BYREF
  unsigned int impostorType; // [esp+24h] [ebp-E8h] BYREF
  int v47; // [esp+28h] [ebp-E4h]
  float *pScalesMax; // [esp+2Ch] [ebp-E0h] BYREF
  float *pScalesMin; // [esp+30h] [ebp-DCh] BYREF
  int *pHeads; // [esp+34h] [ebp-D8h] BYREF
  int *pBodiesMax; // [esp+38h] [ebp-D4h] BYREF
  int *pBodies; // [esp+3Ch] [ebp-D0h] BYREF
  int *pHeadsMax; // [esp+40h] [ebp-CCh] BYREF
  int *pOutfits; // [esp+44h] [ebp-C8h] BYREF
  int *pOutfitsMax; // [esp+48h] [ebp-C4h] BYREF
  EA::ResourceMan::Key modelKey; // [esp+4Ch] [ebp-C0h] BYREF
  cSPVector3 xyz; // [esp+58h] [ebp-B4h]
  cSPVector3 translation; // [esp+64h] [ebp-A8h] BYREF
  cSPVector3 rotation; // [esp+70h] [ebp-9Ch] BYREF
  SP::cTablePropInfo tableInfo[9]; // [esp+7Ch] [ebp-90h] BYREF
  cSPMatrix3 v61; // [esp+E8h] [ebp-24h] BYREF

  SC::cGraphicsInstanced::Clear(this);
  mpObject = this->mNonSimProps.mpObject;
  if ( props != this->mNonSimProps.mpObject )
  {
    if ( props != nullptr )
      props->AddRef(this: props);
    this->mNonSimProps.mpObject = props;
    if ( mpObject != nullptr )
      mpObject->Release(this: mpObject);
  }
  this->mBaseTransform.mRotation = kSPIdentity3_271;
  this->mBaseTransform.mScale = 1.0;
  this->mBaseTransform.mTranslation = kSPZero3_271;
  this->mBaseTransform.mFlags = 0;
  this->mBaseTransform.mModificationCount = 0;
  v5 = this->mNonSimProps.mpObject;
  impostorDataViewType = 1.0;
  if ( v5 != nullptr
    && v5->GetProperty_2(this: v5, a2: 16492049u, a3: (const EA::Variant **)&materialID) != nullptr
    && *(_WORD *)(materialID + 18) == 13 )
  {
    v6 = *EA::Variant::operator<float> float &(this: (EA::Variant *)materialID);
    ++this->mBaseTransform.mModificationCount;
    impostorDataViewType = v6;
    this->mBaseTransform.mScale = v6;
  }
  if ( SP::GetPropertyAsVector3(list: this->mNonSimProps.mpObject, id: 0xFBA610u, result: &translation) != 0 )
  {
    v7 = translation.y * impostorDataViewType;
    v8 = translation.z * impostorDataViewType;
    this->mBaseTransform.mTranslation.x = translation.x * impostorDataViewType;
    this->mBaseTransform.mTranslation.y = v7;
    this->mBaseTransform.mTranslation.z = v8;
    this->mBaseTransform.mFlags |= 4u;
    ++this->mBaseTransform.mModificationCount;
  }
  if ( SP::GetPropertyAsVector3(list: this->mNonSimProps.mpObject, id: 0xFBA613u, result: &rotation) != 0 )
  {
    xyz.x = rotation.x * 0.017453292;
    xyz.y = rotation.y * 0.017453292;
    xyz.z = rotation.z * 0.017453292;
    this->mBaseTransform.mRotation = *SP::Matrix3FromEulerXYZ(result: &v61);
    this->mBaseTransform.mFlags |= 2u;
    ++this->mBaseTransform.mModificationCount;
  }
  v36 = this->mNonSimProps.mpObject;
  modelKeys = nullptr;
  memset(&modelKey, 0, sizeof(modelKey));
  if ( SP::GetPropertyAsKeyArray(list: v36, id: 0xD897169u, count: &numModelKeys, result: &modelKeys) != 0
    || SP::GetPropertyAsKey(list: this->mNonSimProps.mpObject, id: 0xF9EFBBu, result: &modelKey) != 0 )
  {
    if ( modelKeys == nullptr )
    {
      modelKeys = &modelKey;
      numModelKeys = 1;
    }
    v24 = SP::cSimpleUnion<536,8>::Create<SC::cGraphicsInstancedModel>(this: &this->mData);
    if ( (unsigned int)numModelKeys > 4 )
      numModelKeys = 4;
    v40 = this->mNonSimProps.mpObject;
    materialID = 0;
    SP::GetPropertyAsKeyInstance(list: v40, id: 0xCDCCA5Au, result: &materialID);
    SP::cModelInstanceLODSet::SetMaterialID(this: (SP::cModelInstanceLODSet *)v24, matID: materialID);
    *(cSPColorRGBA *)&v24->buffer[488] = kSPColorWhiteA1_264;
    SP::GetPropertyAsColorRGBA(
      list: this->mNonSimProps.mpObject,
      id: 0xFBA612u,
      result: (cSPColorRGBA *)&v24->buffer[488]);
    *(_DWORD *)&v24->buffer[504] = 2139095039;
    *(_DWORD *)&v24->buffer[508] = 2139095039;
    *(_DWORD *)&v24->buffer[512] = 2139095039;
    *(_DWORD *)&v24->buffer[516] = -8388609;
    *(_DWORD *)&v24->buffer[520] = -8388609;
    *(_DWORD *)&v24->buffer[524] = -8388609;
    PropertyAsBoundingBox = SP::GetPropertyAsBoundingBox(
                              list: this->mNonSimProps.mpObject,
                              id: 0xF9EFBAu,
                              result: (cSPBoundingBox *)&v24->buffer[504]);
    *(_DWORD *)&v24->buffer[392] = numModelKeys;
    v26 = 0;
    hasBbox = PropertyAsBoundingBox;
    if ( numModelKeys > 0 )
    {
      v27 = 0;
      v28 = (unsigned int *)&v24->buffer[16];
      do
      {
        *(v28 - 1) = modelKeys[v27].mInstance;
        *v28 = modelKeys[v27].mGroup;
        ++v26;
        ++v27;
        v28 += 24;
      }
      while ( v26 < numModelKeys );
    }
    *(_DWORD *)&v24->buffer[400] = 0;
    SP::GetPropertyAsFloatArray(
      list: this->mNonSimProps.mpObject,
      id: 0x2E33A81u,
      count: (int *)&v24->buffer[400],
      result: (const float **)&v24->buffer[396]);
    *(_DWORD *)&v24->buffer[404] = 1165623296;
    v29 = this->mNonSimProps.mpObject;
    if ( v29 != nullptr
      && v29->GetProperty_2(this: v29, a2: 250029196u, a3: (const EA::Variant **)&impostorDataViewType) != nullptr )
    {
      v30 = (float *)LODWORD(impostorDataViewType);
      if ( *(_WORD *)(LODWORD(impostorDataViewType) + 18) == 13 )
      {
        if ( (*(_BYTE *)(LODWORD(impostorDataViewType) + 16) & 0x30) != 0 )
          v30 = *(float **)LODWORD(impostorDataViewType);
        *(float *)&v24->buffer[404] = *v30;
        goto LABEL_63;
      }
    }
    v31 = *(_DWORD *)&v24->buffer[400];
    if ( v31 <= 0 )
    {
      if ( !hasBbox )
      {
LABEL_63:
        SP::cModelInstanceLODSet::AddToRenderer(
          this: (SP::cModelInstanceLODSet *)v24,
          renderer: effectRenderer,
          drawFlags: 0x88u);
        return 1;
      }
      v33 = *(float *)&v24->buffer[516] - *(float *)&v24->buffer[504];
      v34 = *(float *)&v24->buffer[520] - *(float *)&v24->buffer[508];
      v35 = *(float *)&v24->buffer[524] - *(float *)&v24->buffer[512];
      if ( v33 <= v34 )
      {
        if ( v34 > v35 )
          v35 = *(float *)&v24->buffer[520] - *(float *)&v24->buffer[508];
      }
      else if ( v33 > v35 )
      {
        v35 = *(float *)&v24->buffer[516] - *(float *)&v24->buffer[504];
      }
      v32 = (float)(v35 * *(float *)&v24->buffer[404]) * 0.125;
    }
    else
    {
      v32 = *(float *)(*(_DWORD *)&v24->buffer[396] + 4 * v31 - 4) * 4.0;
    }
    *(float *)&v24->buffer[404] = v32;
    goto LABEL_63;
  }
  v37 = this->mNonSimProps.mpObject;
  impostorType = 0;
  result = SP::GetPropertyAsKeyInstance(list: v37, id: 0xC36D30Du, result: &impostorType);
  if ( result != 0 )
  {
    v38 = this->mNonSimProps.mpObject;
    impostorDataViewType = 0.0;
    if ( SP::GetPropertyAsUint32(list: v38, id: 0xD8C29CFu, result: (unsigned int *)&impostorDataViewType) != 0 )
    {
      v10 = LOWORD(impostorDataViewType);
      v11 = impostorType;
      this->mData.mTypeInfo = (const SP::cSimpleUnionTypeInfo *)&`SP::UnionHelpers::GetTypeInfo<SC::cGraphicsInstancedImpostor>'::`2'::sTypeInfo;
      *(_WORD *)&this->mData.mBuffer.buffer[8] = v10;
      *(_DWORD *)this->mData.mBuffer.buffer = v11;
      *(_WORD *)&this->mData.mBuffer.buffer[10] = 1 << v10;
      v39 = this->mNonSimProps.mpObject;
      impostorDataViewType = 0.0;
      if ( SP::GetPropertyAsKeyInstance(list: v39, id: 0xE701137u, result: (unsigned int *)&impostorDataViewType) != 0 )
        *(float *)&this->mData.mBuffer.buffer[4] = impostorDataViewType;
      else
        *(_DWORD *)&this->mData.mBuffer.buffer[4] = impostorType;
      return 1;
    }
    else
    {
      v12 = SP::cSimpleUnion<536,8>::Create<SC::cGraphicsInstancedSim>(this: &this->mData);
      *(_DWORD *)v12->buffer = impostorType;
      v13 = this->mNonSimProps.mpObject;
      tableInfo[0].mData = &pBodies;
      tableInfo[1].mData = &pBodiesMax;
      tableInfo[2].mData = &pHeads;
      tableInfo[1].mTypeID = -2147483639;
      tableInfo[3].mTypeID = -2147483639;
      tableInfo[5].mTypeID = -2147483639;
      tableInfo[0].mTypeID = 9;
      tableInfo[2].mTypeID = 9;
      tableInfo[3].mData = &pHeadsMax;
      tableInfo[4].mTypeID = 9;
      tableInfo[6].mTypeID = -2147483635;
      tableInfo[7].mTypeID = -2147483635;
      tableInfo[4].mData = &pOutfits;
      tableInfo[5].mData = &pOutfitsMax;
      materialID = (unsigned int)v12;
      tableInfo[0].mPropID = 213722562;
      tableInfo[1].mPropID = 213722565;
      tableInfo[2].mPropID = 213722561;
      tableInfo[3].mPropID = 213722564;
      tableInfo[4].mPropID = 213722563;
      tableInfo[5].mPropID = 213722566;
      tableInfo[6].mPropID = 213722570;
      tableInfo[6].mData = &pScalesMin;
      tableInfo[7].mPropID = 213722571;
      tableInfo[7].mData = &pScalesMax;
      memset(&tableInfo[8], 0, sizeof(SP::cTablePropInfo));
      PropertiesAsTable = SP::GetPropertiesAsTable(props: v13, tablePropInfo: tableInfo);
      eastl::vector<SC::cSimGraphicsInfo,eastl::fixed_compat_allocator>::resize(
        this: (eastl::vector<SC::cDestructionParcel,eastl::fixed_compat_allocator> *)&v12->buffer[4],
        n: PropertiesAsTable);
      v15 = 0;
      impostorDataViewType = 0.0;
      if ( PropertiesAsTable > 0 )
      {
        v47 = 0;
        do
        {
          v16 = (unsigned __int8 *)(v47 + *(_DWORD *)&v12->buffer[4]);
          v16[2] = pBodies[v15];
          *v16 = pHeads[v15];
          v16[4] = pOutfits[v15];
          v16[3] = 1;
          v16[1] = 1;
          v16[5] = 1;
          *((_DWORD *)v16 + 2) = 1065353216;
          *((_DWORD *)v16 + 3) = 1065353216;
          v17 = 1;
          if ( pBodiesMax != nullptr )
          {
            v18 = v16[2];
            if ( pBodiesMax[v15] >= v18 )
            {
              v19 = LOBYTE(pBodiesMax[v15]) - v18 + 1;
              v16[3] = v19;
              v17 = v19;
            }
          }
          if ( pHeadsMax != nullptr && pHeadsMax[v15] >= *v16 )
          {
            v20 = LOBYTE(pHeadsMax[v15]) - *v16 + 1;
            v16[1] = v20;
            v17 *= v20;
          }
          if ( pOutfitsMax != nullptr )
          {
            v21 = v16[4];
            if ( pOutfitsMax[v15] >= v21 )
            {
              v22 = LOBYTE(pOutfitsMax[v15]) - v21 + 1;
              v16[5] = v22;
              v17 *= v22;
            }
          }
          if ( pScalesMin != nullptr )
          {
            v23 = pScalesMin[v15];
            *((float *)v16 + 3) = v23;
            *((float *)v16 + 2) = v23;
          }
          if ( pScalesMax != nullptr )
          {
            *((float *)v16 + 3) = pScalesMax[v15];
            if ( pScalesMin == nullptr )
              *((float *)v16 + 2) = pScalesMax[v15];
          }
          LODWORD(impostorDataViewType) += v17;
          v47 += 16;
          v12 = (eastl::aligned_buffer<536,8> *)materialID;
          ++v15;
        }
        while ( v15 < PropertiesAsTable );
      }
      *(float *)&v12->buffer[20] = impostorDataViewType;
      return 1;
    }
  }
  return result;
}
// --- End Function: ?FillFromProps@cGraphicsInstanced@SC@@QAE_NPAVcPropertyList@SP@@PAVcIRenderer@Swarm@EA@@@Z (0x7872F0) ---
