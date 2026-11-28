"""Run real Simoder codec/mod tests against SDK imports in an isolated copy.

Usage: python scripts/check_simoder_sdk.py D:/rust/packages/Simoder
The source checkout is read-only; generated copies stay under target/.
"""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("simoder", type=Path)
    parser.add_argument("--target", help="Optional Rust target, e.g. i686-pc-windows-msvc")
    args = parser.parse_args()
    source = args.simoder.resolve(strict=True)
    parent = ROOT / "target/sdk-simoder"
    parent.mkdir(parents=True, exist_ok=True)
    # Keep failed runs for inspection, never modify/delete the upstream checkout.
    stage = Path(tempfile.mkdtemp(prefix="consumer-", dir=parent))
    members = ["simoder-core", "simoder-formats", "simoder-mods"]
    for name in members:
        shutil.copytree(source / "crates" / name, stage / "crates" / name)
    if os.name == "nt" and args.target == "i686-pc-windows-msvc":
        # x86 binaries named *patch* trigger Windows installer detection when
        # no execution level is declared. Test executables run as the caller;
        # this does not elevate privileges or change upstream production code.
        build = stage / "crates/simoder-mods/build.rs"
        if build.exists():
            raise RuntimeError("Review existing upstream build.rs before adding test manifests")
        build.write_text('fn main() {\n'
                         ' println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");\n'
                         ' println!("cargo:rustc-link-arg-tests=/MANIFESTUAC:level=\\\'asInvoker\\\'");\n'
                         '}\n', encoding="utf-8")
    manifest = (source / "Cargo.toml").read_text(encoding="utf-8")
    manifest = re.sub(r"members\s*=\s*\[.*?\]", "members = [" + ",".join(
        f'"crates/{name}"' for name in members) + "]", manifest, count=1, flags=re.S)
    for name in ("dbpf", "rw4", "sc-format", "sc-properties"):
        manifest = re.sub(rf"(?m)^{re.escape(name)} = \{{ git = .*\}}$",
                          f'{name} = {{ path = "{(ROOT / "crates" / name).as_posix()}" }}', manifest)
    (stage / "Cargo.toml").write_text(manifest, encoding="utf-8")
    formats = stage / "crates/simoder-formats"
    manifest = (formats / "Cargo.toml").read_text(encoding="utf-8")
    manifest = re.sub(r"(?m)^(dbpf|rw4|sc-format|sc-properties)\.workspace = true\n", "", manifest)
    manifest += f'\nopenscp-sdk = {{ path = "{(ROOT / "crates/openscp-sdk").as_posix()}" }}\n'
    (formats / "Cargo.toml").write_text(manifest, encoding="utf-8")
    lib = (formats / "src/lib.rs").read_text(encoding="utf-8")
    for name in ("dbpf", "rw4", "sc_format", "sc_properties"):
        lib = lib.replace(f"pub use {name};", "")
    lib = lib.replace("use simoder_core::", "pub use openscp_sdk::{package as dbpf, properties as sc_properties, rw4, format as sc_format};\n\nuse simoder_core::", 1)
    (formats / "src/lib.rs").write_text(lib, encoding="utf-8")
    env = dict(os.environ)
    scratch = stage / "temp"
    scratch.mkdir()
    env.update(TEMP=str(scratch), TMP=str(scratch), TMPDIR=str(scratch))
    # Share compilation across isolated copies, but keep it separate from the app.
    env["CARGO_TARGET_DIR"] = str(parent / "build")
    command = ["cargo", "test", "--manifest-path", str(stage / "Cargo.toml"),
               "-p", "simoder-formats", "-p", "simoder-mods", "--lib", "--tests"]
    if args.target:
        command += ["--target", args.target]
    print(f"Simoder consumer copy: {stage}", flush=True)
    subprocess.run(command, cwd=stage, env=env, check=True)


if __name__ == "__main__":
    main()
