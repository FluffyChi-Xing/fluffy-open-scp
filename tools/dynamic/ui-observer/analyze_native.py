"""Summarize captured bridge traffic offline; never attaches to a process."""
import argparse
from collections import Counter, defaultdict
import json
import hashlib
import re
from pathlib import Path


def analyze(directory, ui_script=None):
    status = json.loads((directory / 'status.json').read_text(encoding='utf-8'))
    methods, decoded, prefixes = Counter(), Counter(), defaultdict(Counter)
    examples = defaultdict(dict)
    records = 0
    calls, results, events = {}, [], []
    with (directory / 'trace.jsonl').open(encoding='utf-8') as source:
        for line in source:
            row = json.loads(line)
            records += 1
            method = row.get('name', row['channel'])
            if row['channel'] == 'event':
                events.append(row)
            if row['channel'] == 'result':
                method = 'result:' + method
                results.append(row)
            elif row['channel'] == 'invoke':
                calls[row['sequence']] = row
            methods[method] += 1
            for arg in row.get('arguments', []):
                decoded['/'.join(str(arg.get(k, '')) for k in ('type', 'status', 'reason'))] += 1
            args = row.get('arguments', [])
            if args and args[0].get('status') == 'flat':
                arg = args[0]
                # Preserve the entire suffix: it can contain JSON and embedded slashes.
                text = arg['text']
                prefix, separator, suffix = text.partition('/')
                prefixes[method][prefix] += 1
                examples[method].setdefault(prefix, {
                    'text': text, 'suffix': suffix if separator else None,
                    'truncated': arg['truncated'], 'sequence': row['sequence']})
    paired = []
    result_types = Counter()
    for row in results:
        call = calls.get(row['callSequence'])
        result_types[row['result']['type']] += 1
        if call:
            args = call.get('arguments', [])
            paired.append({'callSequence': row['callSequence'],
                           'query': args[0].get('text') if args else None,
                           'success': row['success'], 'result': row['result']})
    names = defaultdict(list)
    symbol_source = None
    if ui_script is not None:
        raw = ui_script.read_bytes()
        symbol_source = {'path': str(ui_script), 'sha256': hashlib.sha256(raw).hexdigest()}
        for name, number in re.findall(r'((?:simcity|scrui)\.[A-Za-z_][A-Za-z_0-9]*)\s*=\s*(\d+)\s*[;,]', raw.decode('utf-8')):
            if name not in names[int(number)]:
                names[int(number)].append(name)
    event_counts = Counter(row['eventID'] for row in events)
    event_summary = []
    for event_id, count in event_counts.most_common():
        sample = next(row for row in events if row['eventID'] == event_id)
        event_summary.append({'id': event_id, 'count': count, 'names': names[event_id],
                              'sample': sample['event'], 'callerRva': sample['callerRva']})
    return {'events': event_summary, 'symbolSource': symbol_source, 'queryResults': paired, 'resultTypes': dict(result_types),
            'unmatchedResults': len(results) - len(paired), 'records': records, 'methods': dict(methods), 'arguments': dict(decoded),
            'prefixes': {k: dict(v) for k, v in prefixes.items()},
            'examples': dict(examples), 'sampling': status.get('channels', {}),
            'cleanup': {k: status.get(k) for k in ('unloaded', 'detached', 'stopReason')},
            'scope': 'observed_bindings_only; bounded_values; events_from_verified_callers_only'}



def export_samples(directory, output, report):
    rows = [json.loads(line) for line in (directory / 'trace.jsonl').read_text(encoding='utf-8').splitlines()]
    output.mkdir(parents=True, exist_ok=False)
    files = {
        'timeline.json': rows,
        'commands.json': [r for r in rows if r.get('name') == 'PostGameCommand'],
        'queries.json': report['queryResults'],
        'events.json': [r for r in rows if r['channel'] == 'event'],
        'event-catalog.json': report['events'],
        'summary.json': report,
        'status.json': json.loads((directory / 'status.json').read_text(encoding='utf-8')),
    }
    restoration = directory / 'restoration.json'
    if restoration.exists():
        files['restoration.json'] = json.loads(restoration.read_text(encoding='utf-8'))
    manifest = {'source': str(directory.resolve()), 'traceSha256': hashlib.sha256(
        (directory / 'trace.jsonl').read_bytes()).hexdigest(), 'files': {}}
    for name, data in files.items():
        raw = (json.dumps(data, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
        (output / name).write_bytes(raw)
        manifest['files'][name] = {'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}
    (output / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')


def main():
    import sys
    sys.stdout.reconfigure(encoding='utf-8')
    pre = argparse.ArgumentParser(add_help=False)
    pre.add_argument('--lang', choices=['zh-CN', 'en-US'], default='zh-CN')
    lang = pre.parse_known_args()[0].lang
    zh = lang == 'zh-CN'
    parser = argparse.ArgumentParser(add_help=False, description=(
        '离线汇总原生桥接记录。' if zh else 'Summarize native bridge records offline.'))
    parser._positionals.title = '位置参数' if zh else 'Positional arguments'
    parser._optionals.title = '选项' if zh else 'Options'
    parser.add_argument('-h', '--help', action='help', help='显示帮助' if zh else 'Show help')
    parser.add_argument('--lang', choices=['zh-CN', 'en-US'], default=lang,
                        help='输出语言' if zh else 'Output language')
    parser.add_argument('directory', type=Path, help='捕获目录' if zh else 'Capture directory')
    parser.add_argument('--ui-script', type=Path, help='用于事件名称映射的已提取 UI 脚本' if zh else 'Extracted UI script for event name mapping')
    parser.add_argument('--export-dir', type=Path, help='将 JSON 样本导出到新目录' if zh else 'Export JSON samples to a new directory')
    args = parser.parse_args()
    try:
        report = analyze(args.directory, args.ui_script)
        output = args.directory / 'analysis.json'
        output.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding='utf-8')
        if args.export_dir:
            export_samples(args.directory, args.export_dir, report)
    except (OSError, ValueError, KeyError) as error:
        parser.error(('汇总失败：' if zh else 'Analysis failed: ') + str(error))
    print(json.dumps({'output': str(output), 'records': report['records'],
                      'methods': report['methods']}, ensure_ascii=False))


if __name__ == '__main__':
    main()
