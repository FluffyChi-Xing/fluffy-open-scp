/**
 * 性能优化的运行时开关（2026-10-08 EP1 乱码二分取证）：
 * localStorage `openscp.<key>=0` 关闭对应优化项，`=1`/缺省开启。
 * 每一项独立可切——乱码/异常时逐项二分，直接定位到具体优化轮。
 */
function flag(name: string, defaultValue: boolean): boolean {
  try {
    const stored = localStorage.getItem(`openscp.${name}`);
    if (stored !== null) return stored === "1";
    return defaultValue;
  } catch {
    return defaultValue;
  }
}

export const perfFlags = {
  /**
   * 装配尾部 GPU 准备（primeTextures 全量预热 + compileAsync 程序预编译，
   * 完成后才放行首帧）。false = 跳过，首帧走同步编译+按需上传（优化前
   * 形态，首帧会有编译/上传停顿但无窗口期）。
   */
  gpuPrepare: flag("gpuPrepare", true),
  /** primeTextures 全量纹理预热（gpuPrepare 内执行；单独可关）。 */
  primeTextures: flag("primeTextures", true),
  /**
   * 装配三线并发（模型/地面/单元并行，贴花汇合后投影）。
   * false = 串行 await 链（优化前形态，总时长 = 各环节之和）。
   */
  parallelStages: flag("parallelStages", true),
};
