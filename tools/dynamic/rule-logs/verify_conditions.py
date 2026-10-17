"""Recompute eligibility explanations from bounded snapshots and native returns."""
import argparse
import hashlib
import json
from pathlib import Path


def predict(operations):
    count = -1
    for ordinal, op in enumerate(operations):
        if op.get('ordinal') != ordinal:
            return None
        if op.get('status') == 'bin-not-found':
            return dict(count=0, reason='bin-missing', ordinal=ordinal)
        if op.get('status') != 'observed':
            return None
        b = op.get('bin', {})
        amount, capacity, input_value, output = b.get('amount'), b.get('capacity'), op.get('input'), op.get('output')
        if not all(type(v) is int and -2**31 <= v < 2**31 for v in [amount, capacity, input_value, output]):
            return None
        if amount < 0 or capacity < amount or output == -2**31:
            return None
        reason = None
        if output < 0:
            if amount < input_value:
                reason = 'range-below'
            elif amount >= -output:
                reason = 'range-above'
        else:
            if input_value > 0:
                limit = amount // input_value
                count = limit if count < 0 else min(count, limit)
                if count == 0:
                    reason = 'input-shortfall'
            if not reason and output > input_value:
                if output - input_value > 2**31 - 1:
                    return None
                limit = (capacity - amount) // (output - input_value)
                count = limit if count < 0 else min(count, limit)
                if count == 0:
                    reason = 'capacity-shortfall'
        if reason:
            return dict(count=0, reason=reason, ordinal=ordinal)
    return dict(count=count, reason=None)


def verify(rows):
    results, previous = [], 0
    for r in rows:
        if r.get('channel') != 'simulation.condition' or r.get('schema') != 1:
            raise ValueError('unsupported condition event')
        seq = r.get('sequence')
        if type(seq) is not int or seq <= previous:
            raise ValueError('sequence must increase')
        previous = seq
        out = dict(sequence=seq, verified=False, reason='unresolved')
        results.append(out)
        if r.get('readErrors') or type(r.get('nativeAllowed')) is not bool:
            continue
        control = r.get('control', {})
        vectors = [control.get(k) for k in ['actual', 'mask', 'expected']]
        if not all(isinstance(v, list) and len(v) == 2 and all(type(x) is int and 0 <= x < 2**32 for x in v) for v in vectors):
            continue
        matches = all((a & m) == e for a, m, e in zip(*vectors))
        if control.get('matches') is not matches:
            continue
        reason, ordinal = None, None
        if not matches:
            if r['nativeAllowed'] is False and r.get('finalApplyCount') == 0:
                reason = 'control-flags'
        elif r.get('countCapture') == 'captured':
            operations = r.get('operations')
            if not isinstance(operations, list) or len(operations) > 16:
                continue
            p = predict(operations)
            if not p or p['count'] != r.get('resourceCount'):
                continue
            if r['nativeAllowed'] and r.get('finalApplyCount') != 0:
                reason = 'allowed'
            elif r['nativeAllowed'] is False and r.get('finalApplyCount') == 0:
                if p['count'] == 0 and p['reason']:
                    reason, ordinal = p['reason'], p['ordinal']
                elif type(r.get('minApply')) is int and 0 < p['count'] < r['minApply']:
                    reason = 'minimum-apply'
            if r.get('applyResourceId') == '0x00000000':
                final = p['count']
                minimum, maximum = r.get('minApply'), r.get('configuredMaxApply')
                if not all(type(v) is int for v in [minimum, maximum]):
                    continue
                if final > 0:
                    if final < minimum:
                        final = 0
                    if maximum > 0 and final > maximum:
                        final = maximum
                if final != r.get('finalApplyCount') or (final != 0) is not r['nativeAllowed']:
                    continue
        supplied = r.get('diagnosis', {})
        if reason and supplied.get('reason') == reason and (ordinal is None or supplied.get('ordinal') == ordinal):
            out.update(verified=True, reason=reason)
    return results


def summarize(directory):
    raw = (directory / 'trace.jsonl').read_bytes()
    rows = [json.loads(line) for line in raw.splitlines() if line.strip()]
    status = json.loads((directory / 'status.json').read_text(encoding='utf-8'))
    return dict(schema=1, traceSha256=hashlib.sha256(raw).hexdigest(), status=status,
                recordCountMatches=status.get('records') == len(rows), checks=verify(rows))


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('directory', type=Path)
    print(json.dumps(summarize(ap.parse_args().directory), ensure_ascii=False, indent=2))
