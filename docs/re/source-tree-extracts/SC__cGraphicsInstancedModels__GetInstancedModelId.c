/** @file
 *  @brief src/SC/cGraphicsInstancedModels/GetInstancedModelId.c: SC/cGraphicsInstancedModels::GetInstancedModelId - decompiled function 0x787ab0
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?GetInstancedModelId@@cGraphicsInstancedModels@@SC@@@@QAEHABVKey@@ResourceMan@@EA@@@@@@Z
 *  Address: 0x787ab0
 */
// mangled: ?GetInstancedModelId@cGraphicsInstancedModels@SC@@QAEHABVKey@ResourceMan@EA@@@Z
// addr: 0x787ab0
// demangled-sig: int __thiscall SC::cGraphicsInstancedModels::GetInstancedModelId(         SC::cGraphicsInstancedModels *this,         const EA::ResourceMan::Key *key)
// Incoming xrefs for ?GetInstancedModelId@cGraphicsInstancedModels@SC@@QAEHABVKey@ResourceMan@EA@@@Z (0x787AB0): None
// Outgoing xrefs for ?GetInstancedModelId@cGraphicsInstancedModels@SC@@QAEHABVKey@ResourceMan@EA@@@Z (0x787AB0): None
// --- Function: ?GetInstancedModelId@cGraphicsInstancedModels@SC@@QAEHABVKey@ResourceMan@EA@@@Z (0x787AB0) ---
// offset: RVA 0x387AB0 EA 0x787AB0
int __thiscall SC::cGraphicsInstancedModels::GetInstancedModelId(
        SC::cGraphicsInstancedModels *this,
        const EA::ResourceMan::Key *key)
{
  eastl::pair<unsigned __int64,int> *mpEnd; // esi
  eastl::vector_map<unsigned __int64,int,eastl::less<unsigned __int64>,eastl::allocator,eastl::vector<eastl::pair<unsigned __int64,int>,eastl::allocator> > *p_mTable; // ebx
  unsigned int mInstance; // ebp
  eastl::pair<unsigned __int64,int> *v6; // eax
  SP::cIPropertyManager *v8; // esi
  int v9; // esi
  EA::Swarm::cIEffectsWorld_vtbl *v10; // eax
  SP::cPropertyList *mpObject; // ebx
  SC::cGraphicsInstanced *v12; // edi
  EA::Swarm::cIRenderer *v13; // eax
  eastl::pair<unsigned __int64,int> *mpBegin; // [esp-10h] [ebp-48h]
  EA::COM::AutoRefCount<SP::cPropertyList> props; // [esp+10h] [ebp-28h] BYREF
  unsigned __int64 mapKey; // [esp+14h] [ebp-24h] BYREF
  eastl::pair<unsigned __int64,int> value; // [esp+1Ch] [ebp-1Ch] BYREF
  int v18; // [esp+34h] [ebp-4h]

  mpEnd = this->mTable.mpEnd;
  p_mTable = &this->mTable;
  mInstance = key->mInstance;
  HIDWORD(mapKey) = key->mGroup;
  mpBegin = this->mTable.mpBegin;
  LODWORD(mapKey) = mInstance;
  v6 = (eastl::pair<unsigned __int64,int> *)eastl::lower_bound<EA::Callstack::MapFileGCC3::EntryPair *,EA::Callstack::MapFileGCC3::EntryPair,EA::Callstack::MapFileGCC3::EntryPairCompare>(
                                              first: (EA::Callstack::MapFileGCC3::EntryPair *)mpBegin,
                                              last: (EA::Callstack::MapFileGCC3::EntryPair *)mpEnd,
                                              value: (const EA::Callstack::MapFileGCC3::EntryPair *)&mapKey);
  if ( v6 == mpEnd || __PAIR64__(HIDWORD(mapKey), mInstance) < v6->first )
    v6 = mpEnd;
  if ( v6 != this->mTable.mpEnd )
    return v6->second;
  props.mpObject = nullptr;
  v18 = 0;
  v8 = SP::PropertyManager();
  v8->GetPropertyList(this: v8, a2: key->mInstance, a3: key->mGroup, a4: &props.mpObject);
  if ( props.mpObject != nullptr )
  {
    v9 = this->mModels.mpEnd - this->mModels.mpBegin;
    eastl::vector<SC::cGraphicsInstanced,eastl::fixed_compat_allocator>::push_back(this: &this->mModels);
    value.first = __PAIR64__(HIDWORD(mapKey), mInstance);
    value.second = v9;
    eastl::vector_map<unsigned __int64,int,eastl::less<unsigned __int64>,eastl::allocator,eastl::vector<eastl::pair<unsigned __int64,int>,eastl::allocator>>::insert(
      this: p_mTable,
      result: (eastl::pair<eastl::pair<unsigned __int64,int> *,bool> *)&mapKey,
      &value);
    v10 = this->mEffectsWorld.mpObject->__vftable;
    mpObject = props.mpObject;
    v12 = this->mModels.mpEnd - 1;
    v13 = (EA::Swarm::cIRenderer *)((int (*)(void))v10->GetRenderer)();
    SC::cGraphicsInstanced::FillFromProps(this: v12, props: mpObject, effectRenderer: v13);
  }
  else
  {
    v9 = -1;
    value.first = __PAIR64__(HIDWORD(mapKey), mInstance);
    value.second = -1;
    eastl::vector_map<unsigned __int64,int,eastl::less<unsigned __int64>,eastl::allocator,eastl::vector<eastl::pair<unsigned __int64,int>,eastl::allocator>>::insert(
      this: p_mTable,
      result: (eastl::pair<eastl::pair<unsigned __int64,int> *,bool> *)&mapKey,
      &value);
  }
  v18 = -1;
  if ( props.mpObject != nullptr )
    props.mpObject->Release(this: props.mpObject);
  return v9;
}
// --- End Function: ?GetInstancedModelId@cGraphicsInstancedModels@SC@@QAEHABVKey@ResourceMan@EA@@@Z (0x787AB0) ---
