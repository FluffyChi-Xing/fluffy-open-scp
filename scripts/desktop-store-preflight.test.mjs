import { test } from "node:test";
import assert from "node:assert/strict";
import { checkDesktopStoreAccess } from "./desktop-store-preflight.mjs";
test("checks write handles without truncating database files and cleans only its probe", () => {
  const opened=[], removed=[], closed=[];
  checkDesktopStoreAccess("D:/openscp",{
    existsSync:()=>true,
    openSync:(path,flags)=>{opened.push([path,flags]);return opened.length;},
    closeSync:fd=>closed.push(fd),unlinkSync:path=>removed.push(path),
  });
  assert.deepEqual(opened.map(([,flags])=>flags),["r+","r+","r+","wx"]);
  assert.equal(closed.length,4);
  assert.equal(removed.length,1);
  assert.match(removed[0],/\.openscp-write-check-.*\.tmp$/);
});
test("fails before starting Tauri if any existing SQLite file is not writable", () => {
  let count=0;
  assert.throws(()=>checkDesktopStoreAccess("D:/openscp",{
    existsSync:()=>true,openSync:()=>{count++;throw new Error("EPERM");},closeSync:()=>{},unlinkSync:()=>assert.fail("must not remove SQLite files"),
  }),/无法以读写方式打开/);
  assert.equal(count,1);
});
test("detects unwritable parent directory even when existing database handles open", () => {
  assert.throws(()=>checkDesktopStoreAccess("D:/openscp",{
    existsSync:()=>true,openSync:(_path,flags)=>{if(flags==="wx")throw new Error("EPERM");return 1;},
    closeSync:()=>{},unlinkSync:()=>assert.fail("no probe was created"),
  }),/数据库目录无法创建或清理/);
});
test("recognizes low-integrity executable labels without flagging normal ACL entries", async () => {
  const {hasLowIntegrityLabel}=await import("./desktop-store-preflight.mjs");
  assert.equal(hasLowIntegrityLabel("Mandatory Label\\Low Mandatory Level:(I)(NW)"),true);
  assert.equal(hasLowIntegrityLabel("Mandatory Label\\Medium Mandatory Level:(NW)"),false);
  assert.equal(hasLowIntegrityLabel("BUILTIN\\Users:(I)(RX)"),false);
});
