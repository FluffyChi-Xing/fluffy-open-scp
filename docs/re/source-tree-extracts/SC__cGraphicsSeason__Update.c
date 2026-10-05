/** @file
 *  @brief src/SC/cGraphicsSeason/Update.c: SC/cGraphicsSeason::Update - decompiled function 0x690c00
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?Update@@cGraphicsSeason@@SC@@@@QAEXPAVcImpostorRenderer@@2@@IPAVcIEcoGame@@GB@@@@@@Z
 *  Address: 0x690c00
 */
// mangled: ?Update@cGraphicsSeason@SC@@QAEXPAVcImpostorRenderer@2@IPAVcIEcoGame@GB@@@Z
// addr: 0x690c00
// demangled-sig: void __thiscall SC::cGraphicsSeason::Update(         SC::cGraphicsSeason *this,         SC::cImpostorRenderer *pRenderer,         unsigned int impostorClass,         GB::cIEcoGame *pGame)
// Incoming xrefs for ?Update@cGraphicsSeason@SC@@QAEXPAVcImpostorRenderer@2@IPAVcIEcoGame@GB@@@Z (0x690C00): None
// Outgoing xrefs for ?Update@cGraphicsSeason@SC@@QAEXPAVcImpostorRenderer@2@IPAVcIEcoGame@GB@@@Z (0x690C00): None
// --- Function: ?Update@cGraphicsSeason@SC@@QAEXPAVcImpostorRenderer@2@IPAVcIEcoGame@GB@@@Z (0x690C00) ---
// offset: RVA 0x290C00 EA 0x690C00
void __thiscall SC::cGraphicsSeason::Update(
        SC::cGraphicsSeason *this,
        SC::cImpostorRenderer *pRenderer,
        unsigned int impostorClass,
        GB::cIEcoGame *pGame)
{
  double v5; // st7
  char v6; // al
  int v7; // ebp
  signed int v8; // esi
  int v9; // ebx
  float leavesDropped; // [esp+4h] [ebp-4h] BYREF
  bool dayChanged; // [esp+14h] [ebp+Ch]

  v5 = ((double (__thiscall *)(GB::cIEcoGame *, int))pGame->PeriodTime)(a1: pGame, a2: this->mTimePeriodIndex) * 365.0
     + 1.0;
  v6 = SC::cGraphicsSeason::BuildCacheForDay(this, dayOfYear: (int)v5);
  dayChanged = v6;
  if ( v6 != 0 )
  {
    v7 = this->mModelIndices.mpEnd - this->mModelIndices.mpBegin;
    v8 = 0;
    if ( v7 <= 0 )
    {
      this->mDayChanged = v6;
    }
    else
    {
      v9 = 0;
      do
      {
        leavesDropped = 1.0 - this->mColorCache.mpBegin[v9].mLeafAmount;
        SC::cImpostorRenderer::SetOffscreenShaderParameters(
          this: pRenderer,
          classId: impostorClass,
          partIndex: v8++,
          pParameters: &leavesDropped,
          numParameters: 1u);
        ++v9;
      }
      while ( v8 < v7 );
      this->mDayChanged = dayChanged;
    }
  }
  else
  {
    this->mDayChanged = false;
  }
}
// --- End Function: ?Update@cGraphicsSeason@SC@@QAEXPAVcImpostorRenderer@2@IPAVcIEcoGame@GB@@@Z (0x690C00) ---
