"""Read-only audit of native visitor lifetime specialization and host inheritance.

Run with --dump <prop_dump.exe> --native <Scripts.package> --asset <package>
--unit <hex instance>. Emits source hashes and complete parent-chain evidence.
Does not read saves, attach to a process, or modify/deploy packages.
"""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def dump(executable, package, group, instance):
    result = subprocess.run(
        [str(executable), str(package), '00B1B104', group, instance],
        capture_output=True, encoding='utf-8', errors='replace', check=False,
    )
    if result.returncode:
        # Only a missing entry permits resolving the reference in the native pack.
        if 'not found' in result.stderr:
            return None
        raise RuntimeError(result.stderr)
    fields = {}
    for line in result.stdout.splitlines():
        match = re.fullmatch(r'0x([0-9A-F]{8})\s+(\S+)\s+=\s+(.*)', line)
        if match:
            fields.setdefault(match[1], []).append({'type': match[2], 'value': match[3]})
    if not fields and not result.stdout.startswith('found:'):
        raise RuntimeError(f'No property data decoded: {package}/{group}/{instance}')
    return fields


def chain(executable, asset, native, group, instance):
    result, seen = [], set()
    while True:
        identity = (group, instance)
        if identity in seen or len(seen) >= 32:
            raise ValueError('Cyclic or excessive native parent chain')
        seen.add(identity)
        source = asset
        fields = dump(executable, source, group, instance)
        if fields is None and asset != native:
            source = native
            fields = dump(executable, source, group, instance)
        if fields is None:
            raise ValueError(f'Unresolved parent {group}:{instance}')
        result.append({'package': str(source), 'tgi': f'00B1B104:{group}:{instance}',
                       'lifetime': fields.get('0D9114D4'),
                       'parent': fields.get('00B2CCCB')})
        parents = fields.get('00B2CCCB', [])
        if not parents:
            return result
        if len(parents) != 1:
            raise ValueError('Ambiguous parent property')
        parent = re.fullmatch(r'T [0-9A-F]{8} - G ([0-9A-F]{8}) - I ([0-9A-F]{8})', parents[0]['value'])
        if not parent:
            raise ValueError('Unrecognized parent encoding')
        group, instance = parent.groups()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('dump', 'native', 'asset', 'unit'):
        parser.add_argument('--' + option, required=True)
    args = parser.parse_args()
    native, asset = Path(args.native), Path(args.asset)
    native_chain = chain(args.dump, native, native, '40E0C000', '084DB48F')
    asset_chain = chain(args.dump, asset, native, '40E0C000', args.unit.upper())
    effective = lambda rows: next((row['lifetime'] for row in rows if row['lifetime']), None)
    print(json.dumps({
        'native': {'path': str(native), 'sha256': digest(native)},
        'asset': {'path': str(asset), 'sha256': digest(asset)},
        'nativeChain': native_chain, 'assetChain': asset_chain,
        'nativeLifetime': effective(native_chain), 'assetLifetime': effective(asset_chain),
        'scope': 'property inheritance only; no runtime result inferred',
    }, indent=2, ensure_ascii=True))


if __name__ == '__main__':
    main()
