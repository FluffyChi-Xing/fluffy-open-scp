"""Explicit integration test: pass a compiled x86 native_fixture.exe path."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from eawebkit_runtime import Reader
from native_trace import capture


def main():
    executable = Path(sys.argv[1]).resolve()
    process = subprocess.Popen([str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    try:
        addresses = {k: int(v, 16) for k, v in json.loads(process.stdout.readline()).items()}
        reader = Reader(process.pid)
        try:
            signatures = {key: reader.read(addresses[key], 8).hex() for key in ('invoke', 'log', 'event')}
        finally:
            reader.close()
        config = {'exeModule': executable.name, 'dllModule': executable.name,
                  'invokeRva': addresses['invoke'] - addresses['base'], 'logRva': addresses['log'] - addresses['base'],
                  'invokeBytes': signatures['invoke'], 'logBytes': signatures['log'], 'seconds': 6, 'channelLimits': {'command': 80, 'query': 5},
                  'bindings': [{k: hex(addresses[k]) for k in ('object', 'vtable', 'host', 'page')}]}
        reader = Reader(process.pid)
        try:
            site = reader.read(addresses['eventCaller'], 17)
            assert site[:12].hex() == 'ff74240cff74240cff74240c' and site[12] == 0xe8
        finally:
            reader.close()
        config.update({'eventRva': addresses['event'] - addresses['base'],
                       'eventBytes': signatures['event'],
                       'eventSites': [{'rva': addresses['eventCaller'] - addresses['base'],
                                       'returnRva': addresses['eventCaller'] + 17 - addresses['base'],
                                       'bytes': site.hex()}],
                       'objectVtableRva': addresses['objectTable'] - addresses['base'],
                       'arrayVtableRva': addresses['arrayTable'] - addresses['base']})
        config['valueVtableRva'] = addresses['vtable'] - addresses['base']
        config['bindings'][0]['methods'] = ['PostGameCommand', 'RequestGameData']

        def start_work():
            process.stdin.write('\n')
            process.stdin.flush()

        with tempfile.TemporaryDirectory() as directory:
            result = capture(process.pid, config, Path(directory), start_work)
            records = [json.loads(line) for line in (Path(directory) / 'trace.jsonl').read_text(encoding='utf-8').splitlines()]
            outcome = json.loads(process.stdout.readline())
            assert outcome == {'calls': 500, 'bad': 0, 'logs': 1}, outcome
            assert result['ready'] and result['unloaded'] and result['detached'], result
            assert not result['agentErrors'] and result['decodeErrors'] == 0, result
            calls = [x for x in records if x['channel'] == 'invoke']
            assert calls and all(x['argumentCount'] == 4 for x in calls), calls[:1]
            assert len({x['threadId'] for x in calls}) == 2
            assert result['dropped'] > 0
            commands = [x for x in calls if x['name'] == 'PostGameCommand']
            assert len(commands) == 10, commands
            assert result['channels']['command']['dropped'] == 0
            assert result['channels']['query']['dropped'] > 0
            assert all(x['arguments'][0]['text'] == 'camera fixture test' for x in calls)
            assert all(x['arguments'][1]['status'] == 'rope' for x in calls)
            assert all(x['arguments'][2]['reason'] == 'invalid_length' for x in calls)
            assert all(x['arguments'][3]['reason'] == 'unreadable' for x in calls)
            results = [x for x in records if x['channel'] == 'result']
            queries = {x['sequence']: x for x in calls if x['name'] == 'RequestGameData'}
            assert len(results) == len(queries)
            assert len({x['callSequence'] for x in results}) == len(results)
            for row in results:
                assert row['callSequence'] in queries and row['success']
                assert row['result'] in ({'type': 'int32', 'value': row['threadId']}, {'type': 'double', 'value': 42.25})
                assert queries[row['callSequence']]['threadId'] == row['threadId']
            assert {r['result']['type'] for r in results} == {'int32', 'double'}
            events = [r for r in records if r['channel'] == 'event']
            assert len(events) == 9, events
            assert events[0]['event']['items'][1]['items'][1]['status'] == 'depth_limit'
            assert events[1]['event']['status'] == 'sparse_or_lazy'
            assert events[2]['event']['reason'] == 'invalid_array_length'
            for event in events[3:5]:
                obj = event['event']['items'][1]
                assert obj['properties'] == [{'name': 'regionBox', 'value': {'type': 'int32', 'value': 1026}}]
                assert not obj['truncated']
            assert events[5]['event']['items'][1]['status'] == 'accessors_skipped'
            obj = events[6]['event']['items'][1]
            assert obj['properties'][0]['value']['properties'][0]['value']['status'] == 'depth_limit'
            for event in events[7:]:
                assert event['eventID'] == 246368015
                assert event['event']['items'][1]['text'] == 'camera fixture test'
                assert event['event']['length'] == 2 and not event['event']['truncated']
            log = next(x for x in records if x['channel'] == 'console')
            assert log['message']['text'] == 'fixture message' and log['source']['text'] == 'fixture.js'
            assert log['line'] == 42 and log['level'] == 3
            reader = Reader(process.pid)
            try:
                for key in signatures:
                    assert reader.read(addresses[key], 8).hex() == signatures[key], 'hook bytes not restored'
            finally:
                reader.close()
            start_work()
            after = json.loads(process.stdout.readline())
            assert after == {'afterDetachCalls': 520, 'bad': 0}, after
            assert process.wait(timeout=10) == 0
            print(json.dumps({'fixture': 'pass', 'originalCalls': 520, 'records': len(records),
                              'dropped': result['dropped'], 'threads': 2, 'entryBytesRestored': True}))
    finally:
        if process.poll() is None:
            process.terminate()  # Only the fixture created by this test.
            process.wait(timeout=10)


if __name__ == '__main__':
    main()
