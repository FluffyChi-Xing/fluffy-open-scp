/** @file
 *  @brief src/SC/cImpostorRenderer/RegisterImpostorClass.c: SC/cImpostorRenderer::RegisterImpostorClass - decompiled function 0x694c00
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?RegisterImpostorClass@@cImpostorRenderer@@SC@@@@QAE_NIPAVcPropertyList@@SP@@@@@@Z
 *  Address: 0x694c00
 */
// mangled: ?RegisterImpostorClass@cImpostorRenderer@SC@@QAE_NIPAVcPropertyList@SP@@@Z
// addr: 0x694c00
// demangled-sig: char __thiscall SC::cImpostorRenderer::RegisterImpostorClass(         SC::cImpostorRenderer *this,         unsigned int classId,         SP::cPropertyList *pDefinition)
// Incoming xrefs for ?RegisterImpostorClass@cImpostorRenderer@SC@@QAE_NIPAVcPropertyList@SP@@@Z (0x694C00): None
// Outgoing xrefs for ?RegisterImpostorClass@cImpostorRenderer@SC@@QAE_NIPAVcPropertyList@SP@@@Z (0x694C00): None
// --- Function: ?RegisterImpostorClass@cImpostorRenderer@SC@@QAE_NIPAVcPropertyList@SP@@@Z (0x694C00) ---
// offset: RVA 0x294C00 EA 0x694C00
char __thiscall SC::cImpostorRenderer::RegisterImpostorClass(
        SC::cImpostorRenderer *this,
        unsigned int classId,
        SP::cPropertyList *pDefinition)
{
  SP::cPropertyList *v3; // edi
  int v6; // esi
  SC::cImpostorClass **i; // eax
  SC::cImpostorClass *v8; // eax
  SC::cImpostorClass *v9; // edx

  v3 = pDefinition;
  if ( pDefinition == nullptr )
    return 0;
  v6 = 0;
  for ( i = this->mClasses; *i != nullptr; ++i )
  {
    if ( (*i)->mId == classId )
      return SC::cImpostorRenderer::InitImpostorClass(this, pClass: this->mClasses[v6], classId, pProps: pDefinition);
    if ( ++v6 >= 2 )
      return 0;
  }
  v8 = (SC::cImpostorClass *)operator new[](size: 0x2DB4u, name: nullptr, flags: 0);
  v9 = v8;
  if ( v8 != nullptr )
  {
    v8->mId = 0;
    v8->mAggregateBox.mMin.x = 3.4028235e38;
    v8->mAggregateBox.mMin.y = 3.4028235e38;
    v8->mAggregateBox.mMin.z = 3.4028235e38;
    v8->mConfig.mpObject = nullptr;
    v8->mAggregateBox.mMax.x = -3.4028235e38;
    v8->mAggregateBox.mMax.y = -3.4028235e38;
    v8->mAggregateBox.mMax.z = -3.4028235e38;
    v8->mNumAngles = 0;
    v8->mAngleVB = nullptr;
    v8->mRenderOffscreenShadows = false;
    v8->mParts.mpEnd = (SC::cImpostorClassPart *)&v8->mParts.mBuffer;
    v8->mParts.mpBegin = (SC::cImpostorClassPart *)&v8->mParts.mBuffer;
    v8->mParts.mpCapacity = (SC::cImpostorClassPart *)v8->mAttachmentIndex;
    memset(v8->mAttachmentIndex, 0xFFu, sizeof(v8->mAttachmentIndex));
    v3 = pDefinition;
  }
  else
  {
    v9 = nullptr;
  }
  this->mClasses[v6] = v9;
  return SC::cImpostorRenderer::InitImpostorClass(this, pClass: v9, classId, pProps: v3);
}
// --- End Function: ?RegisterImpostorClass@cImpostorRenderer@SC@@QAE_NIPAVcPropertyList@SP@@@Z (0x694C00) ---
