import { checkDesktopStoreAccess, checkDesktopBuildIntegrity } from "./desktop-store-preflight.mjs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { existsSync, readFileSync } from "node:fs";
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
if (process.platform === "win32") {
  try {
    const target = resolve(process.env.CARGO_TARGET_DIR || resolve(root, "target"));
    checkDesktopBuildIntegrity(
      [target, resolve(target, "debug"), resolve(target, "debug", "fluffy-open-scp.exe")],
      existsSync,
      path => spawnSync("icacls.exe", [path], { encoding: "utf8" }),
    );
  } catch (error) {
    console.error([
      "桌面开发版未启动：编译产物被标记为 Low Integrity，Windows 会拒绝它写入正常完整性数据库。",
      error.message,
      "这是 EXE 的完整性标签问题，不是 SQLite 数据损坏；只修改数据库普通用户权限无效。",
      "请先确认并修正该开发 EXE 的完整性标签，保持 D:/openscp/mods 和数据库位置不变。",
    ].join("\n"));
    process.exit(1);
  }
}
// Recompile every local workspace crate; keep only external dependencies.
// This avoids reusing an old app or local asset decoder across launches.
const metadata = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"],
  { cwd: root, encoding: "utf8", env: process.env });
if (metadata.status !== 0) {
  console.error(metadata.stderr || metadata.error || "cargo metadata failed");
  process.exit(1);
}
const workspace = JSON.parse(metadata.stdout);
const members = new Set(workspace.workspace_members);
const packages = workspace.packages.filter(pkg => members.has(pkg.id));
const clean = spawnSync("cargo", ["clean", ...packages.flatMap(pkg => ["--package", pkg.name])],
  { cwd: root, stdio: "inherit", env: process.env });
if (clean.status !== 0) process.exit(clean.status ?? 1);
const packagePath = require.resolve("@tauri-apps/cli/package.json");
const manifest = JSON.parse(readFileSync(packagePath,"utf8"));
const cli = resolve(dirname(packagePath),manifest.bin.tauri);
const args = [cli,"dev","--no-watch", "--config",
  JSON.stringify({build:{beforeDevCommand:"npm run dev -- --force"}})];
args.push(...process.argv.slice(2));
console.log("Rebuilding local Rust crates and starting fresh Vite dependencies before opening the desktop editor.");
const child = spawn(process.execPath,args,{cwd:root,stdio:"inherit",env:{...process.env,CARGO_INCREMENTAL:"0"}});
child.on("error",error=>{console.error(error);process.exitCode=1;});
child.on("exit",code=>{process.exitCode=code??1;});
