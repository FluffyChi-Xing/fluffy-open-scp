"""Summarize saved external snapshots without opening the game process."""
import argparse
from collections import defaultdict
import json
from pathlib import Path

RESOURCES = {
    0x2C1DA112:'Student',0x06EDFBED:'TotalStudents',0xCF0B7832:'StudentStudyingE2',
    0x05DAADA6:'SpotsForKids',0x8E8B4E10:'Power',0x88398207:'PowerNeeded',
    0x9E3C3DFA:'Water',0xCE461A05:'WaterNeeded',0xCD1DDC35:'Sewage',
    0x7CB23A4E:'Garbage',0xFBF3E9D3:'ExpensesFromMaintenance',
    0x5378D6F9:'WorkersNeeded',0x6CCC3A0F:'WorkerW1',0x6CCC3A0C:'WorkerW2',
    0x6CCC3A0D:'WorkerW3',0x1FD156B6:'TotalWorkers',
    0xD3A424B9:'JobsW1',0xD3A424BA:'JobsW2',0xD3A424BB:'JobsW3',
}


def clock(row):
    p=row['periods'];i=p['ids'].index(0x12952639)
    sec=round(p['times'][i]*86400)%86400
    return f'{sec//3600:02d}:{sec//60%60:02d}:{sec%60:02d}'


def analyze(path, start_tick=None, end_tick=None):
    status=json.loads((path/'status.json').read_text())
    raw_rows=[json.loads(x) for x in (path/'snapshots.jsonl').read_text().splitlines()]
    selected=[(i,row) for i,row in enumerate(raw_rows)
              if (start_tick is None or row['tickBefore']>=start_tick)
              and (end_tick is None or row['tickBefore']<=end_tick)]
    if not selected:
        raise ValueError('empty_capture_window')
    rows=[row for _,row in selected]
    mapping={idx:rid for rid,idx in json.loads((path/'resource-map.json').read_text())}
    previous={};amounts=defaultdict(list);changes=[];counter_resets=[];triggers=[];old_states={}
    # Preserve the incoming transition at the window boundary. Counts and
    # resource ranges still describe only the selected samples.
    predecessor=raw_rows[selected[0][0]-1] if selected[0][0]>0 else None
    if predecessor:
        previous={RESOURCES[mapping[b['resourceIndex']]]:b['amount'] for b in predecessor['bins']
                  if mapping[b['resourceIndex']] in RESOURCES}
        old_states={s['ruleId']:s for s in predecessor.get('simStates',[])}
    invariants=[];phase_diffs=[]
    for number,row in selected:
        current={}
        for b in row['bins']:
            name=RESOURCES.get(mapping[b['resourceIndex']])
            if not name:continue
            amount=b['amount'];current[name]=amount;amounts[name].append(amount)
            if name in previous and amount!=previous[name]:
                changes.append(dict(sample=number,utc=row['utc'],tick=row['tickBefore'],tickAfter=row['tickAfter'],
                                    clock=clock(row),resource=name,before=previous[name],after=amount,
                                    delta=amount-previous[name]))
        previous=current
        if 'Student' in current and 'SpotsForKids' in current:
            invariants.append(current['Student']+current['SpotsForKids'])
        for state in row.get('simStates',[]):
            rid=state['ruleId'];old=old_states.get(rid)
            if old:
                if state['triggerTimeIndex']<0 and state['simCounter']>old['simCounter']:
                    counter_resets.append(dict(sample=number,tick=row['tickBefore'],tickAfter=row['tickAfter'],
                                               clock=clock(row),ruleId=rid,before=old['simCounter'],after=state['simCounter']))
                if state['triggerCount']!=old['triggerCount']:
                    triggers.append(dict(sample=number,tick=row['tickBefore'],clock=clock(row),ruleId=rid,
                                         before=old['triggerCount'],after=state['triggerCount'],
                                         nextTriggerTime=state['nextTriggerTime']))
            old_states[rid]=state
        p=row['periods'];di=p['ids'].index(0x12952639)
        expected=row['tickBefore']*p['inverseLengths'][di]+p['phases'][di]
        actual=p['cycles'][di]+p['times'][di]
        phase_diffs.append((actual-expected)*1440)
    summary={name:dict(first=vs[0],last=vs[-1],minimum=min(vs),maximum=max(vs),
                       observedDecreases=sum(-c['delta'] for c in changes if c['resource']==name and c['delta']<0),
                       observedIncreases=sum(c['delta'] for c in changes if c['resource']==name and c['delta']>0))
             for name,vs in amounts.items()}
    correlations=[]
    for reset in counter_resets:
        resource={'0xd51cb94c':'Power','0xd54f8b18':'Water'}.get(reset['ruleId'])
        matched=[c for c in changes if c['resource']==resource and c['delta']<0 and abs(c['tick']-reset['tick'])<=1]
        correlations.append(dict(reset=reset,nearbyDecreases=matched,
                                 attribution='temporal correlation in external snapshots, not function-level effect trace'))
    return dict(path=str(path),status=status,samples=len(rows),firstTick=rows[0]['tickBefore'],
                captureFinalized='finishedUtc' in status,
                sourceSamples=len(raw_rows),
                windowSelection=dict(startTick=start_tick,endTick=end_tick,
                                     predecessorTick=predecessor['tickBefore'] if predecessor else None,
                                     eventSampleIndex='original capture line, zero-based'),
                statusSampleCountMatches=status['samples']==len(raw_rows),
                lastTick=rows[-1]['tickAfter'],firstClock=clock(rows[0]),lastClock=clock(rows[-1]),
                unstableSamples=sum(not r['stableIdentity'] for r in rows),
                tickCrossingSamples=sum(not r['sameTick'] for r in rows),
                studentPlusSeatsRange=[min(invariants),max(invariants)] if invariants else None,
                clockInterpolationInTicks=[min(phase_diffs),max(phase_diffs)],
                resources=summary,changes=changes,counterResets=counter_resets,
                hourlyTriggerAdvances=triggers,correlations=correlations,
                limitations=['non-atomic snapshots; intermediate mutations can be missed',
                             'observed decreases/increases are not exhaustive consumption/production totals',
                             'live period clock is not a synchronized HUD reading',
                             'scheduler trigger advancement does not prove resource conditions passed'])


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path)
    ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--start-tick',type=int)
    ap.add_argument('--end-tick',type=int)
    args=ap.parse_args()
    result=analyze(args.capture,args.start_tick,args.end_tick)
    args.out.write_text(json.dumps(result,indent=2)+'\n',encoding='utf8')
    print(json.dumps({k:result[k] for k in ['samples','firstTick','lastTick','firstClock','lastClock',
                                          'counterResets','hourlyTriggerAdvances']},indent=2))


if __name__=='__main__':main()
