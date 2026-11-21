import { checkDesktopStoreAccess, hasLowIntegrityLabel } from "./desktop-store-preflight.mjs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";

const require = createRequire(import.meta.url);
const root = resolve(import.meta.dirname, "..");
try {
  checkDesktopStoreAccess("D:/openscp");
} catch (error) {
  console.error([
    "桌面开发版未启动：当前进程不能写入 D:/openscp 的数据库或 WAL/SHM。",
    error.message,
    "请在具有 D:/openscp 写权限的普通 Windows PowerShell/Terminal 中运行 pnpm run dev:desktop。",
    "若当前终端由受限任务启动，请改用任务外的终端；管理员身份不是必要条件。",
    "不会切换到旧开发数据库，也不会删除数据库、WAL 或 SHM。",
  ].join("\n"));
  process.exit(1);
}
const nativeExecutable = resolve(root, "target", "debug", "fluffy-open-scp.exe");
if (process.platform === "win32" && existsSync(nativeExecutable)) {
  const labels = spawnSync("icacls.exe", [nativeExecutable], {encoding:"utf8"});
  if (labels.status === 0 && hasLowIntegrityLabel(labels.stdout)) {
    console.error([
      "桌面开发版未启动：编译产物被标记为 Low Integrity，Windows 会拒绝它写入正常完整性数据库。",
      nativeExecutable,
      "这是 EXE 的完整性标签问题，不是 SQLite 数据损坏；只修改数据库普通用户权限无效。",
      "请先确认并修正该开发 EXE 的完整性标签，保持 D:/openscp/mods 和数据库位置不变。",
    ].join("\n"));
    process.exit(1);
  }
}
let reuse = false;
try {
  const response = await fetch("http://127.0.0.1:5173/@vite/client", {signal:AbortSignal.timeout(1500)});
  reuse = response.ok && (await response.text()).includes("vite");
} catch { /* Tauri will start the configured frontend server. */ }
const packagePath = require.resolve("@tauri-apps/cli/package.json");
const manifest = JSON.parse(readFileSync(packagePath,"utf8"));
const cli = resolve(dirname(packagePath),manifest.bin.tauri);
const args = [cli,"dev","--no-watch"];
if (reuse) {
  const config = resolve(root,"tmp","desktop-dev","tauri.json");
  mkdirSync(dirname(config),{recursive:true});
  writeFileSync(config,JSON.stringify({build:{beforeDevCommand:null}}));
  args.push("--config",config);
}
args.push(...process.argv.slice(2));
console.log(reuse ? "Reusing Vite at http://127.0.0.1:5173; starting native editor with HMR." : "Starting native editor and Vite with HMR.");
const child = spawn(process.execPath,args,{cwd:root,stdio:"inherit",env:process.env});
child.on("error",error=>{console.error(error);process.exitCode=1;});
child.on("exit",code=>{process.exitCode=code??1;});
