import * as fs from "node:fs";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

/** Non-destructive permission check: existing SQLite files are never written. */
export function checkDesktopStoreAccess(root, io = fs) {
  for (const suffix of ["", "-wal", "-shm"]) {
    const path = join(root, "openscp.db" + suffix);
    if (!io.existsSync(path)) continue;
    let handle;
    try {
      handle = io.openSync(path, "r+");
    } catch (cause) {
      throw new Error("无法以读写方式打开 " + path, {cause});
    } finally {
      if (handle !== undefined) io.closeSync(handle);
    }
  }
  const probe = join(root, ".openscp-write-check-" + randomUUID() + ".tmp");
  let handle, created = false;
  try {
    handle = io.openSync(probe, "wx");
    created = true;
    io.closeSync(handle);
    handle = undefined;
    io.unlinkSync(probe);
    created = false;
  } catch (cause) {
    throw new Error("数据库目录无法创建或清理 SQLite 辅助文件：" + root, {cause});
  } finally {
    if (handle !== undefined) io.closeSync(handle);
    if (created) { try { io.unlinkSync(probe); } catch { /* original error is preserved */ } }
  }
}
/** icacls resolves the mandatory SID to the Windows display language. */
export function hasLowIntegrityLabel(output) {
  return /Mandatory Label[\\/]Low Mandatory Level|S-1-16-4096|强制标签[\\/]低强制级别|强制性标签[\\/]低强制性级别/i.test(output);
}
