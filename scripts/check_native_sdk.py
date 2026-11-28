"""Exercise the SDK feature and downstream dependency contracts without the app."""
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
CONSUMER = ROOT / "crates/openscp-sdk/tests/consumer/Cargo.toml"


def cargo(*args, capture=False):
    return subprocess.run(
        ["cargo", *args], cwd=ROOT, check=True, text=True,
        stdout=subprocess.PIPE if capture else None,
    ).stdout


def main():
    for features in (None, "", "package", "properties", "rw4",
                     "package,properties", "package,rw4", "properties,rw4"):
        options = [] if features is None else ["--no-default-features"]
        if features:
            options += ["--features", features]
        cargo("test", "--locked", "-p", "openscp-sdk", *options)
    cargo("test", "--locked", "-p", "dbpf", "-p", "sc-properties", "-p", "rw4", "-p", "sc-format")
    cargo("test", "--manifest-path", str(CONSUMER))
    # Inspect normal edges only: sc-properties intentionally uses registry in tests.
    tree = cargo("tree", "--manifest-path", str(CONSUMER), "--edges", "normal", "--prefix", "none", capture=True)
    names = {line.split()[0] for line in tree.splitlines() if line.strip()}
    forbidden = {name for name in names if name.startswith("tauri") or name in {
        "rusqlite", "libsqlite3-sys", "sc-registry", "sc-store", "fluffy-open-scp",
    }}
    if forbidden:
        raise RuntimeError(f"SDK pulled host dependencies: {sorted(forbidden)}")
    print(json.dumps({"sdk_checks": "passed", "normal_dependency_packages": len(names)}))


if __name__ == "__main__":
    main()
