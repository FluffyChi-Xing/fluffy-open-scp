"""One-time import of the existing local DOM capture profile; never changes game packages."""
import argparse,json,shutil
from pathlib import Path

def migrate(source, runtime, game):
    if not game.is_file():raise ValueError('Game executable missing')
    key=(source/'live/session-key').read_text(encoding='ascii').strip()
    if len(key)!=48 or any(c not in '0123456789abcdefABCDEF' for c in key):raise ValueError('Invalid credential')
    pages={'game://localhost/resource/0x40464200!0x82c29d9b.0xdd6233d6'}
    pages.update((source/'000_OpenSCP_CityHostDom.uris').read_text(encoding='ascii').splitlines())
    if not all(p.startswith('game://localhost/resource/') for p in pages):raise ValueError('Invalid allowlist')
    config={'gameExecutable':str(game.resolve()),'allowedPages':sorted(pages)}
    if (runtime/'capture.json').exists():
        if json.loads((runtime/'capture.json').read_text(encoding='utf8'))!=config:raise ValueError('Existing capture profile differs; not overwritten')
        if (runtime/'live/session-key').read_text().strip()!=key:raise ValueError('Existing credential differs; not overwritten')
        return
    (runtime/'live').mkdir(parents=True,exist_ok=True)
    (runtime/'live/session-key').write_text(key,encoding='ascii')
    for frame in (source/'live').glob('overlay-*.json'):
        data=json.loads(frame.read_bytes())
        if frame.name!=f"overlay-{data['pid']}.json":raise ValueError('Snapshot identity mismatch')
        destination=runtime/'live'/frame.name
        if not destination.exists():shutil.copyfile(frame,destination)
    (runtime/'capture.json').write_text(json.dumps(config,ensure_ascii=False,indent=2),encoding='utf8')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--source',type=Path,required=True);parser.add_argument('--runtime',type=Path,required=True);parser.add_argument('--game',type=Path,required=True)
    args=parser.parse_args();migrate(args.source,args.runtime,args.game)
    print('Capture profile imported; game packages unchanged')
