/** @file
 *  @brief src/SC/cGraphicsInstancedImpostor/GetImpostorInfo.c: SC/cGraphicsInstancedImpostor::GetImpostorInfo - decompiled function 0x695510
 *
 *  Verbatim CodeDumper body below; only this header is new.
 *  Mangled: ?GetImpostorInfo@@cGraphicsInstancedImpostor@@SC@@@@QAEIPAVcGraphicsGame@@2@@I@@Z
 *  Address: 0x695510
 */
// mangled: ?GetImpostorInfo@cGraphicsInstancedImpostor@SC@@QAEIPAVcGraphicsGame@2@I@Z
// addr: 0x695510
// demangled-sig: unsigned int __thiscall SC::cGraphicsInstancedImpostor::GetImpostorInfo(         SC::cGraphicsInstancedImpostor *this,         SC::cGraphicsGame *pGfx,         unsigned int randomBits)
// Incoming xrefs for ?GetImpostorInfo@cGraphicsInstancedImpostor@SC@@QAEIPAVcGraphicsGame@2@I@Z (0x695510): None
// Outgoing xrefs for ?GetImpostorInfo@cGraphicsInstancedImpostor@SC@@QAEIPAVcGraphicsGame@2@I@Z (0x695510): None
// --- Function: ?GetImpostorInfo@cGraphicsInstancedImpostor@SC@@QAEIPAVcGraphicsGame@2@I@Z (0x695510) ---
// offset: RVA 0x295510 EA 0x695510
unsigned int __thiscall SC::cGraphicsInstancedImpostor::GetImpostorInfo(
        SC::cGraphicsInstancedImpostor *this,
        SC::cGraphicsGame *pGfx,
        unsigned int randomBits)
{
  int mModelType; // esi
  SC::cTreeEnvironment *v4; // ecx

  if ( pGfx != nullptr
    && (mModelType = this->mModelType) < (unsigned int)(pGfx->mSeasonInfo.mColorCache.mpEnd
                                                      - pGfx->mSeasonInfo.mColorCache.mpBegin) )
  {
    v4 = &pGfx->mSeasonInfo.mColorCache.mpBegin[mModelType];
  }
  else
  {
    v4 = &SC::kDefaultTreeEnvironemnt;
  }
  return (v4->mHsvMin[2] + ((((randomBits >> 10) & 7) * (v4->mHsvMax[2] - v4->mHsvMin[2])) >> 3))
       | (32 * (((((randomBits >> 7) & 7) * (v4->mHsvMax[1] - v4->mHsvMin[1])) & 0xFFFFFFF8) + 8 * v4->mHsvMin[1]))
       | ((((((randomBits >> 4) & 7) * (v4->mHsvMax[0] - v4->mHsvMin[0])) & 0xFFFFFFF8) + 8 * v4->mHsvMin[0]) << 13);
}
// --- End Function: ?GetImpostorInfo@cGraphicsInstancedImpostor@SC@@QAEIPAVcGraphicsGame@2@I@Z (0x695510) ---
