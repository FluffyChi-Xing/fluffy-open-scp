/** @file
 *  @brief src/SC/cTerrainForest2/GetForestTexture.c: SC/cTerrainForest2::GetForestTexture - decompiled function 0x42fe90
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?GetForestTexture@@cTerrainForest2@@SC@@@@QBE?AV?$AutoRefCount@@VcTextureInstance@@SP@@@@@@EA@@@@H@@Z
 *  Address: 0x42fe90
 */
// mangled: ?GetForestTexture@cTerrainForest2@SC@@QBE?AV?$AutoRefCount@VcTextureInstance@SP@@@EA@@H@Z
// addr: 0x42fe90
// demangled-sig: EA::AutoRefCount<SP::cTextureInstance> *__thiscall SC::cTerrainForest2::GetForestTexture(         SC::cTerrainForest2 *this,         EA::AutoRefCount<SP::cTextureInstance> *result,         int index)
// Incoming xrefs for ?GetForestTexture@cTerrainForest2@SC@@QBE?AV?$AutoRefCount@VcTextureInstance@SP@@@EA@@H@Z (0x42FE90): None
// Outgoing xrefs for ?GetForestTexture@cTerrainForest2@SC@@QBE?AV?$AutoRefCount@VcTextureInstance@SP@@@EA@@H@Z (0x42FE90): None
// --- Function: ?GetForestTexture@cTerrainForest2@SC@@QBE?AV?$AutoRefCount@VcTextureInstance@SP@@@EA@@H@Z (0x42FE90) ---
// offset: RVA 0x2FE90 EA 0x42FE90
EA::AutoRefCount<SP::cTextureInstance> *__thiscall SC::cTerrainForest2::GetForestTexture(
        SC::cTerrainForest2 *this,
        EA::AutoRefCount<SP::cTextureInstance> *result,
        int index)
{
  _DWORD *v4; // eax
  int v5; // eax
  SP::cTextureInstance *mpObject; // ecx
  SP::cITextureManager *v8; // esi
  unsigned int v9; // eax
  SP::cTextureInstance *v10; // eax

  v4 = SP::ShaderData(id: 0x201u);
  if ( v4 != nullptr
    && ((v5 = v4[1]) == 1 || v5 == 7)
    && (mpObject = this->mRenderTargets.mpBegin[index].mNormalMap.mpObject) != nullptr
    || (mpObject = this->mRenderTargets.mpBegin[index].mColorMap.mpObject) != nullptr )
  {
    result->mpObject = mpObject;
    _InterlockedExchangeAdd(&mpObject->mRefCount.mValue, 1u);
    return result;
  }
  else
  {
    v8 = SP::TextureManager();
    v9 = EA::StdC::FNV1_String8(pData8: "DefaultBlack", nInitialValue: 0x811C9DC5, charCase: kCharCaseLower);
    v10 = v8->GetTexture(this: v8, a2: v9, a3: 1073751808u, a4: kUseFlagsDefault);
    result->mpObject = v10;
    if ( v10 != nullptr )
      _InterlockedExchangeAdd(&v10->mRefCount.mValue, 1u);
    return result;
  }
}
// --- End Function: ?GetForestTexture@cTerrainForest2@SC@@QBE?AV?$AutoRefCount@VcTextureInstance@SP@@@EA@@H@Z (0x42FE90) ---
