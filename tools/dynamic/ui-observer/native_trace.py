"""Bounded native UI observation with verified hooks; never calls game APIs."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys
import threading
import time

from eawebkit_runtime import inspect, Reader, validate_view

TEXT = {
    "zh-CN": {"description": "EAWebKit 最小原生观察：3 个入口，限时自动撤回。",
              "pid": "游戏进程 ID", "directory": "新建捕获目录", "seconds": "观察秒数（5–300）",
              "help": "显示帮助并退出", "lang": "输出语言", "options": "选项",
              "error": "观察失败", "ready": "原生观察器已就绪", "done": "观察已结束",
              "scan_incomplete": "网页对象扫描未完成，请稍后重试；诊断已保留",
              "binding_missing": "未找到通过校验的网页绑定，请等待游戏界面加载完成后重试",
              "event_site": "事件调用点校验失败", "host_binding": "网页宿主绑定已变化，请重试",
              "event_entry": "事件入口字节不匹配，可能已有其他观察器运行",
              "invoke_entry": "调用或日志入口字节不匹配，可能已有其他观察器运行", "duration": "观察时长须为 5–300 秒"},
    "en-US": {"description": "Minimal native EAWebKit observation: three entrypoints, automatic timed removal.",
              "pid": "Game process ID", "directory": "New capture directory", "seconds": "Capture seconds (5–300)",
              "help": "Show help and exit", "lang": "Output language", "options": "Options",
              "error": "Observation failed", "ready": "Native observer ready", "done": "Observation ended",
              "scan_incomplete": "Page object scan did not finish. Retry shortly; diagnostics were retained",
              "binding_missing": "No validated page binding found. Wait for the game UI to load and retry",
              "event_site": "Event call site validation failed", "host_binding": "Page host binding changed; retry",
              "event_entry": "Event entry bytes do not match; another observer may be active",
              "invoke_entry": "Invoke or log entry bytes do not match; another observer may be active", "duration": "Capture duration must be 5–300 seconds"},
}
PROFILES = {
    'balanced': {},
    'ui-events': {'event': 240, 'region': 120, 'regionResult': 120},
    'bridge-commands': {'command': 240, 'query': 120, 'result': 120},
    'engine-messages': {'console': 240, 'other': 160, 'result': 120},
}
OBSERVATIONS = {
    'ui:all': {'scope': 'ui', 'target': 'all'},
    'building:school': {'scope': 'building', 'target': 'school'},
    'building:omegaco': {'scope': 'building', 'target': 'omegaco'},
    'agent:police': {'scope': 'agent', 'target': 'police'},
}


def prepare(pid, seconds, directory, profile='balanced', observation='ui:all'):
    if not 5 <= seconds <= 300:
        raise ValueError('duration')
    if profile not in PROFILES:
        raise ValueError('profile')
    if observation not in OBSERVATIONS:
        raise ValueError('observation')
    survey = inspect(pid, views=True)
    candidates = survey.get('views', {}).get('candidates', [])
    bindings = []
    for item in candidates:
        bound = item.get('clientBinding', {})
        if bound.get('location') != {'module': 'SimCity.exe', 'rva': '0x632930'}:
            continue
        bindings.append({'object': bound['object'], 'vtable': bound['vtable'], 'host': item['userData'],
                         'page': item['page'], 'uri': item.get('currentFrameURI'),
                         'methods': [x['name'] for x in item['registeredCallbacks']]})
    if not bindings:
        directory.parent.mkdir(parents=True, exist_ok=True)
        directory.with_suffix('.preflight.json').write_text(json.dumps(survey, indent=2, ensure_ascii=False), encoding='utf-8')
        raise ValueError('scan_incomplete' if survey.get('views', {}).get('stopReason') not in (None, 'complete') else 'binding_missing')
    # Only fixed, independently disassembled function starts. Never hook shared ret4.
    reader = Reader(pid)
    try:
        modules = {m['name'].lower(): m for m in reader.modules}
        invoke = reader.read(modules['simcity.exe']['base'] + 0x632930, 8).hex()
        logger = reader.read(modules['eawebkit.dll']['base'] + 0x5AC490, 8).hex()
        event = reader.read(modules['simcity.exe']['base'] + 0x62F500, 8).hex()
        sites = [{'rva': rva, 'returnRva': rva + 21,
                  'bytes': '8b4c24148b168b52508d44241c506a00518bceffd2'}
                 for rva in (0x22DC76, 0x369D67)]
        for site in sites:
            if reader.read(modules['simcity.exe']['base'] + site['rva'], 21).hex() != site['bytes']:
                raise ValueError('event_site')
        for binding in bindings:
            host = int(binding['host'], 16)
            table = int.from_bytes(reader.read(host, 4), 'little')
            target = int.from_bytes(reader.read(table + 0x50, 4), 'little')
            if target != modules['simcity.exe']['base'] + 0x62F500:
                raise ValueError('host_binding')
        if event != '538bd98b4c240885':
            raise ValueError('event_entry')
        if invoke != '8b4904e968f4ffff' or logger != '83ec385355568bf1':
            raise ValueError('invoke_entry')
    finally:
        reader.close()
    config = {'exeModule': 'SimCity.exe', 'dllModule': 'EAWebKit.dll',
              'eventRva': 0x62F500, 'eventBytes': event, 'eventSites': sites,
              'objectVtableRva': 0x777B08, 'arrayVtableRva': 0x870CD0, 'arrayDepth': 6, 'objectDepth': 7, 'valueBudget': 256, 'invokeRva': 0x632930, 'logRva': 0x5AC490, 'invokeBytes': invoke,
              'valueVtableRva': 0x778A88, 'logBytes': logger, 'bindings': bindings, 'seconds': seconds, 'refreshBindings': True,
              'profile': profile, 'channelLimits': PROFILES[profile], 'observation': OBSERVATIONS[observation]}
    directory.mkdir(parents=True, exist_ok=False)
    (directory / 'preflight.json').write_text(json.dumps(survey, indent=2, ensure_ascii=False), encoding='utf-8')
    (directory / 'config.json').write_text(json.dumps(config, indent=2, ensure_ascii=False), encoding='utf-8')
    return config


def validate_candidate(pid, host_text):
    """Read-only validation outside game callbacks; never trust a reported address."""
    reader = Reader(pid)
    try:
        modules = {m['name'].lower(): m for m in reader.modules}
        if 'eawebkit.dll' not in modules or 'simcity.exe' not in modules:
            return None
        host = int(host_text, 16)
        view = reader.u32(host + 0xB0)
        item = validate_view(reader, view, modules['eawebkit.dll']['base'] + 0x85C2B8)
        if not item or int(item['userData'], 16) != host:
            return None
        bound = item.get('clientBinding', {})
        if bound.get('location') != {'module': 'SimCity.exe', 'rva': '0x632930'}:
            return None
        if reader.u32(reader.u32(host) + 0x50) != modules['simcity.exe']['base'] + 0x62F500:
            return None
        return {'object': bound['object'], 'vtable': bound['vtable'], 'host': item['userData'],
                'view': item['view'], 'private': item['private'], 'page': item['page'],
                'uri': item.get('currentFrameURI'), 'methods': [x['name'] for x in item['registeredCallbacks']]}
    finally:
        reader.close()


def capture(pid, config, directory, ready_callback=lambda: None):
    import frida
    lock = threading.RLock()
    stopped = threading.Event()
    summary = {'pid': pid, 'profile': config.get('profile', 'balanced'), 'observation': config.get('observation', {'scope': 'ui', 'target': 'all'}), 'ready': False, 'records': 0, 'counts': {}, 'agentErrors': [],
               'dropped': 0, 'decodeErrors': 0, 'unloaded': False, 'detached': False,
               'fridaVersion': frida.__version__, 'runtime': 'QJS', 'passiveObservation': True,
               'readOnlyProcess': False}
    counts = Counter()
    session = script = None
    total_bytes = 0
    pending_hosts = set()
    attempted_hosts = {}
    summary['bindingRefresh'] = []

    def save():
        temporary = directory / 'status.tmp'
        temporary.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding='utf-8')
        temporary.replace(directory / 'status.json')

    def on_message(message, _data):
        nonlocal total_bytes
        with lock:
            if message.get('type') == 'error':
                summary['agentErrors'].append(message.get('description', '')[:512])
                stopped.set()
                save()
                return
            if message.get('type') != 'send':
                return
            payload = message['payload']
            kind = payload.get('kind')
            for host in payload.get('candidateHosts', [])[:16]:
                if config.get('refreshBindings') and isinstance(host, str) and len(pending_hosts) < 16:
                    pending_hosts.add(host)
            if kind == 'ready':
                summary['ready'] = True
                ready_callback()
            rows = payload.get('records', [])
            observation = config.get('observation', {})
            for row in rows:
                row['observationScope'] = observation.get('scope', 'ui')
                row['observationTarget'] = observation.get('target', 'all')
                if observation.get('scope') == 'building' and row.get('channel') == 'result':
                    request = str(row.get('requestName', ''))
                    if request in {'resourceAmount', 'binAmount'}:
                        row['semantic'] = 'resource_change'
                elif observation.get('scope') == 'agent' and row.get('channel') == 'event':
                    row['semantic'] = 'dispatch_event'
            encoded = ''.join(json.dumps(x, ensure_ascii=False) + '\n' for x in rows).encode('utf-8')
            if total_bytes + len(encoded) > config.get('diskLimitBytes', 8 * 1024 * 1024):
                summary['stopReason'] = 'disk_limit'
                stopped.set()
            elif encoded:
                with (directory / 'trace.jsonl').open('ab') as output:
                    output.write(encoded)
                total_bytes += len(encoded)
                summary['records'] += len(rows)
                for row in rows:
                    key = row.get('name', row['channel'])
                    if row['channel'] == 'result':
                        key = 'result:' + key
                    counts[key] += 1
                summary['counts'] = dict(counts)
            summary['diagnostics'] = payload.get('diagnostics', summary.get('diagnostics', {}))
            summary['channels'] = payload.get('channels', summary.get('channels', {}))
            summary['dropped'] = payload.get('dropped', summary['dropped'])
            summary['decodeErrors'] = payload.get('errors', summary['decodeErrors'])
            summary['hits'] = payload.get('hits', summary.get('hits', 0))
            summary['errorExamples'] = payload.get('errorExamples', summary.get('errorExamples', []))
            if kind == 'stopped':
                summary.setdefault('stopReason', payload.get('reason'))
                stopped.set()
            save()
            if kind == 'batch' and script is not None:
                script.post({'type': 'ack'})

    def on_detached(reason, crash=None):
        with lock:
            summary['detachedReason'] = reason
            summary['crashReported'] = crash is not None
            stopped.set()
            save()

    try:
        # Attach once per run, use QJS, then unload and detach in the same session.
        session = frida.get_local_device().attach(pid)
        session.on('detached', on_detached)
        source = 'globalThis.OPEN_SCP_NATIVE_CONFIG=' + json.dumps(config) + ';\n'
        source += Path(__file__).with_name('native_agent.js').read_text(encoding='utf-8')
        (directory / 'agent.js').write_text(source, encoding='utf-8')
        summary['agentSha256'] = hashlib.sha256(source.encode('utf-8')).hexdigest()
        script = session.create_script(source, runtime='qjs')
        script.on('message', on_message)
        script.load()
        deadline = time.monotonic() + config['seconds'] + 5
        while not stopped.wait(0.25) and time.monotonic() < deadline:
            if directory.with_suffix('.stop').exists():
                summary['stopReason'] = 'requested'
                break
            with lock:
                candidates = list(pending_hosts)[:4]
                pending_hosts.difference_update(candidates)
            for host in candidates:
                now = time.monotonic()
                if now - attempted_hosts.get(host, 0) < 2 or len(summary['bindingRefresh']) >= 64:
                    continue
                attempted_hosts[host] = now
                try:
                    binding = validate_candidate(pid, host)
                    accepted = bool(binding and script.exports_sync.addbinding(binding))
                    entry = {'time': int(time.time() * 1000), 'host': host, 'accepted': accepted, 'binding': binding}
                except Exception as error:
                    entry = {'time': int(time.time() * 1000), 'host': host, 'accepted': False, 'error': str(error)[:160]}
                with lock:
                    summary['bindingRefresh'].append(entry)
                    save()
    except KeyboardInterrupt:
        summary['stopReason'] = 'interrupted'
    finally:
        if script is not None:
            try:
                script.exports_sync.stop()
            except Exception as error:
                summary['stopError'] = str(error)[:256]
            try:
                script.unload()
                summary['unloaded'] = True
            except Exception as error:
                summary['unloadError'] = str(error)[:256]
        if session is not None:
            try:
                session.detach()
                summary['detached'] = True
            except Exception as error:
                summary['detachError'] = str(error)[:256]
        with lock:
            save()
    return summary


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
    pre = argparse.ArgumentParser(add_help=False)
    pre.add_argument('--lang', choices=TEXT, default='zh-CN')
    selected, _ = pre.parse_known_args()
    labels = TEXT[selected.lang]
    parser = argparse.ArgumentParser(add_help=False, description=labels['description'])
    parser._optionals.title = labels['options']
    parser.add_argument('-h', '--help', action='help', help=labels['help'])
    parser.add_argument('--lang', choices=TEXT, default=selected.lang, help=labels['lang'])
    parser.add_argument('--pid', type=int, required=True, help=labels['pid'])
    parser.add_argument('--directory', type=Path, required=True, help=labels['directory'])
    parser.add_argument('--seconds', type=int, default=60, help=labels['seconds'])
    parser.add_argument('--profile', choices=sorted(PROFILES), default='balanced')
    parser.add_argument('--observation', choices=sorted(OBSERVATIONS), default='ui:all')
    parser.add_argument('--exhaustive', action='store_true', help='保留所有已挂接入口记录，不做每秒采样')
    args = parser.parse_args()
    try:
        config = prepare(args.pid, args.seconds, args.directory, args.profile, args.observation)
        if args.exhaustive:
            config['channelMaxLimit'] = 1000000
            config['channelLimits'] = {name: 1000000 for name in (
                'command', 'query', 'other', 'console', 'result', 'event',
                'mutation', 'mutationResult', 'template', 'templateResult',
                'region', 'regionResult')}
            config['diskLimitBytes'] = 128 * 1024 * 1024
            config['channelQueueLimit'] = 1000000
            config['batchLimit'] = 4096
            config['finalBatchLimit'] = 1000000
        if args.directory.with_suffix('.stop').exists():
            (args.directory / 'status.json').write_text(json.dumps({'ready': False, 'stopReason': 'requested', 'unloaded': True, 'detached': True}), encoding='utf-8')
            return
        summary = capture(args.pid, config, args.directory, lambda: print(labels['ready'], flush=True))
    except Exception as error:
        print(json.dumps({'code': str(error), 'message': labels['error'] + ': ' + labels.get(str(error), str(error))}, ensure_ascii=False), file=sys.stderr)
        raise SystemExit(1)
    print(json.dumps({'message': labels['done'], **summary}, ensure_ascii=False))
    if not summary['ready'] or summary['agentErrors'] or not summary['unloaded'] or not summary['detached']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
