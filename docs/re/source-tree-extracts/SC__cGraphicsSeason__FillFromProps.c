/** @file
 *  @brief src/SC/cGraphicsSeason/FillFromProps.c: SC/cGraphicsSeason::FillFromProps - decompiled function 0x6903a0
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?FillFromProps@@cGraphicsSeason@@SC@@@@QAEXPAVcPropertyList@@SP@@@@PAVcIEcoGame@@GB@@@@@@Z
 *  Address: 0x6903a0
 */
// mangled: ?FillFromProps@cGraphicsSeason@SC@@QAEXPAVcPropertyList@SP@@PAVcIEcoGame@GB@@@Z
// addr: 0x6903a0
// demangled-sig: void __thiscall SC::cGraphicsSeason::FillFromProps(         SC::cGraphicsSeason *this,         SP::cPropertyList *pProps,         GB::cIEcoGame *pGame)
// Incoming xrefs for ?FillFromProps@cGraphicsSeason@SC@@QAEXPAVcPropertyList@SP@@PAVcIEcoGame@GB@@@Z (0x6903A0): None
// Outgoing xrefs for ?FillFromProps@cGraphicsSeason@SC@@QAEXPAVcPropertyList@SP@@PAVcIEcoGame@GB@@@Z (0x6903A0): None
// --- Function: ?FillFromProps@cGraphicsSeason@SC@@QAEXPAVcPropertyList@SP@@PAVcIEcoGame@GB@@@Z (0x6903A0) ---
// offset: RVA 0x2903A0 EA 0x6903A0
void __thiscall SC::cGraphicsSeason::FillFromProps(
        SC::cGraphicsSeason *this,
        SP::cPropertyList *pProps,
        GB::cIEcoGame *pGame)
{
  eastl::vector<SC::cTreeEnvironment,eastl::fixed_compat_allocator> *p_mTreeEnvironment; // esi
  SC::cTreeEnvironment *mpBegin; // edx
  SP::cPropertyList *v6; // ecx
  SP::cPropertyList *mpObject; // ebx
  int v8; // eax
  unsigned int v9; // ebx
  int PropertiesAsTable; // eax
  int v11; // ecx
  int v12; // edi
  unsigned int v13; // edx
  float v14; // xmm3_4
  __int128 v15; // xmm5
  int v16; // edi
  eastl::rbtree_node_base **v17; // eax
  EA::Text::LineBreakIterator *mInstance; // ecx
  int v19; // kr00_4
  float v20; // xmm1_4
  float v21; // xmm2_4
  float v22; // xmm7_4
  float v23; // xmm6_4
  float v24; // xmm7_4
  float v25; // xmm6_4
  float v26; // xmm0_4
  float v27; // xmm1_4
  int v28; // xmm7_4
  float v29; // xmm6_4
  float v30; // xmm2_4
  unsigned int v31; // ecx
  SC::cTreeEnvironment *mpEnd; // eax
  int v33; // eax
  float v34; // xmm2_4
  int v35; // ecx
  int v36; // edx
  float v37; // xmm2_4
  float v38; // xmm0_4
  SC::cTreeEnvironment *v39; // eax
  int v40; // ecx
  int v41; // esi
  int v42; // esi
  unsigned int v43; // edx
  eastl::rbtree_node_base *v44; // eax
  eastl::rbtree_node_base **p_mpNodeLeft; // ecx
  eastl::rbtree_node_base *v46; // eax
  eastl::rbtree_node_base **p_mpNodeRight; // ecx
  const SC::cTornado::cParticleAnimatedBone::cParticleIndexPair *v48; // eax
  SC::cTornado::cParticleAnimatedBone::cParticleIndexPair *v49; // ecx
  SC::cTornado::cParticleAnimatedBone::cParticleIndexPair *v50; // eax
  int v51; // esi
  int v52; // edi
  EA::Text::LineBreakIterator *key; // [esp+D9Ch] [ebp-120h] BYREF
  EA::Variant *v54; // [esp+DA0h] [ebp-11Ch] BYREF
  unsigned int v55; // [esp+DA4h] [ebp-118h] BYREF
  int v56; // [esp+DA8h] [ebp-114h] BYREF
  EA::ResourceMan::Key *v57; // [esp+DACh] [ebp-110h] BYREF
  unsigned int v58; // [esp+DB0h] [ebp-10Ch]
  float v59; // [esp+DB4h] [ebp-108h]
  eastl::rbtree<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,eastl::less<EA::Internet::HTTPServerJob *>,eastl::allocator,eastl::use_self<EA::Internet::HTTPServerJob *>,0,1> v60; // [esp+DB8h] [ebp-104h] BYREF
  eastl::vector<SC::cTornado::cParticleAnimatedBone::cParticleIndexPair,eastl::fixed_compat_allocator> *p_mModelIndices; // [esp+DD8h] [ebp-E4h]
  float v62; // [esp+DDCh] [ebp-E0h]
  float v63; // [esp+DE0h] [ebp-DCh]
  float v64; // [esp+DE4h] [ebp-D8h]
  float v65; // [esp+DE8h] [ebp-D4h]
  float v66; // [esp+DECh] [ebp-D0h]
  float v67; // [esp+DF0h] [ebp-CCh]
  float v68; // [esp+DF4h] [ebp-C8h]
  int v69; // [esp+DF8h] [ebp-C4h] BYREF
  float v70; // [esp+DFCh] [ebp-C0h]
  int v71; // [esp+E00h] [ebp-BCh] BYREF
  SC::cTornado::cParticleAnimatedBone::cParticleIndexPair result; // [esp+E04h] [ebp-B8h] BYREF
  float v73; // [esp+E0Ch] [ebp-B0h]
  float v74; // [esp+E10h] [ebp-ACh]
  int v75; // [esp+E14h] [ebp-A8h] BYREF
  float v76[5]; // [esp+E18h] [ebp-A4h] BYREF
  float v77; // [esp+E2Ch] [ebp-90h]
  float v78; // [esp+E30h] [ebp-8Ch]
  SP::cTablePropInfo tablePropInfo; // [esp+E34h] [ebp-88h] BYREF
  int v80; // [esp+E40h] [ebp-7Ch]
  int v81; // [esp+E44h] [ebp-78h]
  int *v82; // [esp+E48h] [ebp-74h]
  int v83; // [esp+E4Ch] [ebp-70h]
  int v84; // [esp+E50h] [ebp-6Ch]
  int *v85; // [esp+E54h] [ebp-68h]
  int v86; // [esp+E58h] [ebp-64h]
  int v87; // [esp+E5Ch] [ebp-60h]
  float *v88; // [esp+E60h] [ebp-5Ch]
  int v89; // [esp+E64h] [ebp-58h]
  int v90; // [esp+E68h] [ebp-54h]
  int *v91; // [esp+E6Ch] [ebp-50h]
  int v92; // [esp+E70h] [ebp-4Ch]
  int v93; // [esp+E74h] [ebp-48h]
  __int64 v94; // [esp+E78h] [ebp-44h]
  SC::cParcelOverlapInfo value; // [esp+E80h] [ebp-3Ch] BYREF
  __int128 v96; // [esp+E8Ch] [ebp-30h]
  __int128 v97; // [esp+E9Ch] [ebp-20h]
  int v98; // [esp+EB8h] [ebp-4h]

  this->mTreeEnvironment.mpEnd = this->mTreeEnvironment.mpBegin;
  p_mTreeEnvironment = &this->mTreeEnvironment;
  this->mModelIndices.mpEnd = this->mModelIndices.mpBegin;
  mpBegin = this->mColorCache.mpBegin;
  v6 = pProps;
  this->mColorCache.mpEnd = mpBegin;
  mpObject = this->mTreeSeasonInfoPropList.mpObject;
  v55 = (unsigned int)this;
  p_mModelIndices = (eastl::vector<SC::cTornado::cParticleAnimatedBone::cParticleIndexPair,eastl::fixed_compat_allocator> *)&this->mModelIndices;
  this->mCurrentDay = -1;
  this->mTimePeriodIndex = 0;
  if ( pProps != mpObject )
  {
    if ( pProps != nullptr )
    {
      pProps->AddRef(this: pProps);
      v6 = pProps;
    }
    this->mTreeSeasonInfoPropList.mpObject = v6;
    if ( mpObject != nullptr )
    {
      mpObject->Release(this: mpObject);
      v6 = pProps;
    }
  }
  if ( v6 != nullptr )
  {
    if ( v6->GetProperty_2(this: v6, a2: 235641346u, a3: &v54) != nullptr
      && v54->mTypeId == 1
      && *EA::Variant::operator<bool> bool &(this: v54) )
    {
      v9 = 0;
    }
    else
    {
      v8 = pGame->IndexFromPeriodID(this: pGame, a2: -1685338578u);
      v9 = 0;
      this->mTimePeriodIndex = v8;
      if ( v8 < 0 )
        this->mTimePeriodIndex = 0;
    }
    v60.mAnchor.mpNodeLeft = (eastl::rbtree_node_base *)&v60.mAnchor.mpNodeLeft;
    v60.mAnchor.mpNodeParent = (eastl::rbtree_node_base *)&v60.mAnchor.mpNodeLeft;
    memset(&v60.mAnchor.mColor, 0, 12);
    tablePropInfo.mData = &v57;
    v84 = 49;
    v87 = 49;
    v85 = &v69;
    v88 = v76;
    v82 = &v75;
    v98 = 0;
    tablePropInfo.mPropID = 235641341;
    tablePropInfo.mTypeID = 32;
    v80 = 235641342;
    v81 = 9;
    v83 = 235641343;
    v86 = 235641344;
    v89 = 235641347;
    v90 = 13;
    v91 = &v71;
    v92 = 0;
    v93 = 0;
    LODWORD(v94) = 0;
    PropertiesAsTable = SP::GetPropertiesAsTable(props: pProps, &tablePropInfo);
    v56 = PropertiesAsTable;
    if ( PropertiesAsTable >= 0 )
    {
      v11 = -1;
      v12 = 0;
      v13 = 0;
      key = (EA::Text::LineBreakIterator *)-1;
      v54 = nullptr;
      if ( PropertiesAsTable > 0 )
      {
        v14 = 0.0;
        v15 = 0;
        *(float *)&v15 = 255.0;
        v96 = 0u;
        v97 = v15;
        while ( 1 )
        {
          if ( v11 != v57[v9 / 0xC].mInstance )
          {
            if ( v11 != -1 && v12 != p_mTreeEnvironment->mpEnd - p_mTreeEnvironment->mpBegin )
            {
              eastl::vector<SC::cTreeEnvironment,eastl::fixed_compat_allocator>::push_back(
                this: p_mTreeEnvironment,
                value: &p_mTreeEnvironment->mpBegin[v12]);
              *(_WORD *)(*(_DWORD *)(v55 + 8) - 12) += 365;
              eastl::rbtree<unsigned int,eastl::pair<unsigned int const,EA::AutoRefCount<SP::cILightingWorld>>,eastl::less<unsigned int>,eastl::allocator,eastl::use_first<eastl::pair<unsigned int const,EA::AutoRefCount<SP::cILightingWorld>>>,1,1>::find(
                this: (eastl::rbtree<EA::Text::LineBreakIterator *,eastl::pair<EA::Text::LineBreakIterator * const,EA::Text::TextRun *>,eastl::less<EA::Text::LineBreakIterator *>,EA::WebKitUtil::EASTLAllocator,eastl::use_first<eastl::pair<EA::Text::LineBreakIterator * const,EA::Text::TextRun *> >,1,1> *)&v60.mAnchor,
                (eastl::rbtree_iterator<EA::AutoRefCount<SP::cICheatConsole>,EA::AutoRefCount<SP::cICheatConsole> const *,EA::AutoRefCount<SP::cICheatConsole> const &> *)&result,
                &key);
              *eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator>::operator[](
                 this: (eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator> *)&v60.mAnchor,
                 (const unsigned int *)&key) = (eastl::rbtree_node_base *)v12;
              v16 = p_mTreeEnvironment->mpEnd - p_mTreeEnvironment->mpBegin;
              v17 = eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator>::operator[](
                      this: (eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator> *)&v60.mAnchor,
                      (const unsigned int *)&key);
              LODWORD(v15) = v97;
              v14 = *(float *)&v96;
              v17[1] = (eastl::rbtree_node_base *)v16;
            }
            mInstance = (EA::Text::LineBreakIterator *)v57[v9 / 0xC].mInstance;
            v19 = (char *)p_mTreeEnvironment->mpEnd - (char *)p_mTreeEnvironment->mpBegin;
            *(_DWORD *)&v60.mCompare.eastl::binary_function<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,bool> = 0;
            v12 = v19 / 12;
            v13 = 0;
            key = mInstance;
          }
          v20 = *(float *)(v9 + v69 + 4) - SC::kSeasonTintRangeMin.y;
          v21 = *(float *)(v9 + v69 + 8) - SC::kSeasonTintRangeMin.z;
          v22 = *(float *)(v9 + LODWORD(v76[0]) + 4);
          v66 = (float)(*(float *)(v9 + v69) - SC::kSeasonTintRangeMin.x)
              / (float)(SC::kSeasonTintRangeMax.x - SC::kSeasonTintRangeMin.x);
          v76[1] = v20;
          v68 = v20 / (float)(SC::kSeasonTintRangeMax.y - SC::kSeasonTintRangeMin.y);
          v76[3] = v21;
          v70 = v21 / (float)(SC::kSeasonTintRangeMax.z - SC::kSeasonTintRangeMin.z);
          v23 = *(float *)(v9 + LODWORD(v76[0])) - SC::kSeasonTintRangeMin.x;
          v77 = v22;
          v24 = *(float *)(v9 + LODWORD(v76[0]) + 8);
          v62 = v23;
          v78 = v24;
          v74 = v77 - SC::kSeasonTintRangeMin.y;
          v25 = v24;
          v76[2] = SC::kSeasonTintRangeMax.x - SC::kSeasonTintRangeMin.x;
          v26 = v62 / (float)(SC::kSeasonTintRangeMax.x - SC::kSeasonTintRangeMin.x);
          v65 = SC::kSeasonTintRangeMax.y - SC::kSeasonTintRangeMin.y;
          v27 = (float)(v77 - SC::kSeasonTintRangeMin.y)
              / (float)(SC::kSeasonTintRangeMax.y - SC::kSeasonTintRangeMin.y);
          *(float *)&v28 = 0.0;
          v29 = (float)(v25 - SC::kSeasonTintRangeMin.z)
              / (float)(SC::kSeasonTintRangeMax.z - SC::kSeasonTintRangeMin.z);
          v67 = SC::kSeasonTintRangeMax.z - SC::kSeasonTintRangeMin.z;
          v30 = *(float *)(v71 + 4 * (_DWORD)v54);
          v64 = v26;
          v63 = v27;
          v73 = v29;
          v59 = v30;
          if ( v30 < 0.0 || (*(float *)&v28 = 1.0, v30 > 1.0) )
            v59 = *(float *)&v28;
          v31 = *(_DWORD *)(v75 + 4 * (_DWORD)v54);
          v58 = v31;
          if ( v31 < v13 || v31 == 0 || v31 > 0x16D )
          {
            v58 = v13;
            v31 = v13;
          }
          mpEnd = p_mTreeEnvironment->mpEnd;
          *(_DWORD *)&v60.mCompare.eastl::binary_function<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,bool> = v31;
          if ( mpEnd >= p_mTreeEnvironment->mpCapacity )
          {
            memset(&value, 0, sizeof(value));
            eastl::vector<SC::cErrorEntry,eastl::fixed_compat_allocator>::DoInsertValue(
              this: (eastl::vector<SC::cParcelOverlapInfo,eastl::fixed_compat_allocator> *)p_mTreeEnvironment,
              position: (SC::cParcelOverlapInfo *)mpEnd,
              &value);
            LODWORD(v15) = v97;
            v14 = *(float *)&v96;
            v29 = v73;
            v27 = v63;
            v26 = v64;
            LOWORD(v31) = v58;
          }
          else
          {
            p_mTreeEnvironment->mpEnd = mpEnd + 1;
            if ( mpEnd != nullptr )
            {
              *(_QWORD *)&mpEnd->mDay = 0;
              mpEnd->mLeafAmount = 0.0;
            }
          }
          v33 = *(_DWORD *)(v55 + 8) - 12;
          v34 = fmaxf(fminf(v66 * 255.0, *(float *)&v15), v14);
          *(_WORD *)v33 = v31;
          v35 = (int)v34;
          v36 = (int)fmaxf(fminf(v68 * 255.0, *(float *)&v15), v14);
          v37 = v70 * 255.0;
          *(_BYTE *)(v33 + 2) = v35;
          *(_BYTE *)(v33 + 3) = v36;
          *(_BYTE *)(v33 + 4) = (int)fmaxf(fminf(v37, *(float *)&v15), v14);
          *(_BYTE *)(v33 + 5) = (int)fmaxf(fminf(v26 * 255.0, *(float *)&v15), v14);
          v38 = v59;
          *(_BYTE *)(v33 + 6) = (int)fmaxf(fminf(v27 * 255.0, *(float *)&v15), v14);
          *(_BYTE *)(v33 + 7) = (int)fmaxf(fminf(v29 * 255.0, *(float *)&v15), v14);
          *(float *)(v33 + 8) = v38;
          v9 += 12;
          v54 = (EA::Variant *)((char *)v54 + 1);
          if ( (int)v54 >= v56 )
            break;
          v13 = *(_DWORD *)&v60.mCompare.eastl::binary_function<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,bool>;
          v11 = (int)key;
        }
        if ( key != (EA::Text::LineBreakIterator *)-1 && v12 != p_mTreeEnvironment->mpEnd - p_mTreeEnvironment->mpBegin )
        {
          v39 = p_mTreeEnvironment->mpEnd;
          v40 = (int)&p_mTreeEnvironment->mpBegin[v12];
          if ( v39 >= p_mTreeEnvironment->mpCapacity )
          {
            eastl::vector<SC::cErrorEntry,eastl::fixed_compat_allocator>::DoInsertValue(
              this: (eastl::vector<SC::cParcelOverlapInfo,eastl::fixed_compat_allocator> *)p_mTreeEnvironment,
              position: (SC::cParcelOverlapInfo *)v39,
              value: (const SC::cParcelOverlapInfo *)&p_mTreeEnvironment->mpBegin[v12]);
          }
          else
          {
            p_mTreeEnvironment->mpEnd = v39 + 1;
            if ( v39 != nullptr )
            {
              *(_QWORD *)&v39->mDay = *(_QWORD *)v40;
              v39->mLeafAmount = *(float *)(v40 + 8);
            }
          }
          *(_WORD *)(*(_DWORD *)(v55 + 8) - 12) += 365;
          *eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator>::operator[](
             this: (eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator> *)&v60.mAnchor,
             (const unsigned int *)&key) = (eastl::rbtree_node_base *)v12;
          v41 = p_mTreeEnvironment->mpEnd - p_mTreeEnvironment->mpBegin;
          eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator>::operator[](
            this: (eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator> *)&v60.mAnchor,
            (const unsigned int *)&key)[1] = (eastl::rbtree_node_base *)v41;
        }
      }
      if ( SP::GetPropertyAsKeyArray(
             list: pProps,
             id: 0xE0B9A01u,
             count: &v56,
             result: (const EA::ResourceMan::Key **)&v57) != 0 )
      {
        v54 = nullptr;
        if ( v56 > 0 )
        {
          v42 = 0;
          do
          {
            v43 = v57[v42].mInstance;
            v44 = *(eastl::rbtree_node_base **)&v60.mAnchor.mColor;
            v55 = v43;
            p_mpNodeLeft = &v60.mAnchor.mpNodeLeft;
            if ( *(_DWORD *)&v60.mAnchor.mColor == 0 )
              goto LABEL_52;
            do
            {
              if ( v44[1].mpNodeRight < (eastl::rbtree_node_base *)v43 )
              {
                v44 = v44->mpNodeRight;
              }
              else
              {
                p_mpNodeLeft = &v44->mpNodeRight;
                v44 = v44->mpNodeLeft;
              }
            }
            while ( v44 != nullptr );
            if ( p_mpNodeLeft == &v60.mAnchor.mpNodeLeft || v43 < (unsigned int)p_mpNodeLeft[4] )
            {
LABEL_52:
              v43 = 782826392;
              v55 = 782826392;
            }
            v46 = *(eastl::rbtree_node_base **)&v60.mAnchor.mColor;
            p_mpNodeRight = &v60.mAnchor.mpNodeLeft;
            if ( *(_DWORD *)&v60.mAnchor.mColor == 0 )
              goto LABEL_64;
            do
            {
              if ( v46[1].mpNodeRight < (eastl::rbtree_node_base *)v43 )
              {
                v46 = v46->mpNodeRight;
              }
              else
              {
                p_mpNodeRight = &v46->mpNodeRight;
                v46 = v46->mpNodeLeft;
              }
            }
            while ( v46 != nullptr );
            if ( p_mpNodeRight == &v60.mAnchor.mpNodeLeft || v43 < (unsigned int)p_mpNodeRight[4] )
            {
LABEL_64:
              v50 = p_mModelIndices->mpEnd;
              if ( v50 >= p_mModelIndices->mpCapacity )
              {
                result.mFragmentIndex = 0;
                result.mParticleIndex = 0;
                eastl::vector<SC::cPathDistanceEntry,eastl::fixed_compat_allocator>::DoInsertValue(
                  this: p_mModelIndices,
                  position: v50,
                  value: &result);
              }
              else
              {
                p_mModelIndices->mpEnd = v50 + 1;
                if ( v50 != nullptr )
                {
                  v50->mFragmentIndex = 0;
                  v50->mParticleIndex = 0;
                }
              }
            }
            else
            {
              v48 = (const SC::cTornado::cParticleAnimatedBone::cParticleIndexPair *)eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator>::operator[](
                                                                                       this: (eastl::map<unsigned int,SC::cGraphicsSeason::cTreeDataIndex,eastl::less<unsigned int>,eastl::allocator> *)&v60.mAnchor,
                                                                                       key: &v55);
              v49 = p_mModelIndices->mpEnd;
              if ( v49 >= p_mModelIndices->mpCapacity )
              {
                eastl::vector<SC::cPathDistanceEntry,eastl::fixed_compat_allocator>::DoInsertValue(
                  this: p_mModelIndices,
                  position: v49,
                  value: v48);
              }
              else
              {
                p_mModelIndices->mpEnd = v49 + 1;
                if ( v49 != nullptr )
                {
                  v49->mFragmentIndex = v48->mFragmentIndex;
                  v49->mParticleIndex = v48->mParticleIndex;
                }
              }
            }
            ++v42;
            v54 = (EA::Variant *)((char *)v54 + 1);
          }
          while ( (int)v54 < v56 );
        }
      }
      v51 = *(_DWORD *)&v60.mAnchor.mColor;
      v98 = -1;
      if ( *(_DWORD *)&v60.mAnchor.mColor != 0 )
      {
        do
        {
          eastl::rbtree<eastl::pair<int,int>,eastl::pair<int,int>,eastl::less<eastl::pair<int,int>>,eastl::allocator,eastl::use_self<eastl::pair<int,int>>,0,1>::DoNukeSubtree(
            this: (eastl::rbtree<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,eastl::less<EA::Internet::HTTPServerJob *>,eastl::allocator,eastl::use_self<EA::Internet::HTTPServerJob *>,0,1> *)&v60.mAnchor,
            pNode: *(eastl::rbtree_node<EA::Internet::HTTPServerJob *> **)v51);
          v52 = *(_DWORD *)(v51 + 4);
          operator delete(p: (void *)v51);
          v51 = v52;
        }
        while ( v52 != 0 );
      }
    }
    else
    {
      v98 = -1;
      eastl::rbtree<eastl::pair<int,int>,eastl::pair<int,int>,eastl::less<eastl::pair<int,int>>,eastl::allocator,eastl::use_self<eastl::pair<int,int>>,0,1>::DoNukeSubtree(
        this: (eastl::rbtree<EA::Internet::HTTPServerJob *,EA::Internet::HTTPServerJob *,eastl::less<EA::Internet::HTTPServerJob *>,eastl::allocator,eastl::use_self<EA::Internet::HTTPServerJob *>,0,1> *)&v60.mAnchor,
        pNode: *(eastl::rbtree_node<EA::Internet::HTTPServerJob *> **)&v60.mAnchor.mColor);
    }
  }
}
// --- End Function: ?FillFromProps@cGraphicsSeason@SC@@QAEXPAVcPropertyList@SP@@PAVcIEcoGame@GB@@@Z (0x6903A0) ---
