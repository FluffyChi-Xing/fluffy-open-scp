export interface MotionTimeline {
  duration: number;
  tracks: {
    slot: number;
    start: number;
    end: number;
    from: number;
    to: number;
    looping: boolean;
    pingPong: boolean;
  }[];
}

/** Sample the model rows in authored order, holding completed tracks for scrubbing. */
export function sampleMotionTimeline(timeline: MotionTimeline, time: number): number[] {
  const values = [0, 0, 0, 0];
  const sequenceTime = Math.min(timeline.duration, Math.max(0, time));
  for (const track of timeline.tracks) {
    if (sequenceTime < track.start) continue;
    const elapsed = (sequenceTime - track.start) / (track.end - track.start);
    let phase = Math.min(1, elapsed);
    if (track.pingPong) {
      const cycle = track.looping ? elapsed % 2 : Math.min(elapsed, 2);
      phase = cycle <= 1 ? cycle : 2 - cycle;
    } else if (track.looping) {
      phase = elapsed % 1;
    }
    values[track.slot] = track.from + (track.to - track.from) * phase;
  }
  return values;
}
