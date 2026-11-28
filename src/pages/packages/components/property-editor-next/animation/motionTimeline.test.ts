import { expect, it } from "vitest";
import { sampleMotionTimeline } from "./motionTimeline";

it("holds each completed track while other parts continue, and supports backwards scrubbing", () => {
  const timeline = {
    duration: 4,
    tracks: [
      { slot: 0, start: 0, end: 1, from: 0, to: 1, looping: false, pingPong: false },
      { slot: 1, start: 0, end: 4, from: 0, to: 1, looping: false, pingPong: false },
      { slot: 0, start: 3, end: 4, from: 1, to: 0.98, looping: false, pingPong: false },
    ],
  };
  expect(sampleMotionTimeline(timeline, 2)).toEqual([1, 0.5, 0, 0]);
  expect(sampleMotionTimeline(timeline, 8)).toEqual([0.98, 1, 0, 0]);
  expect(sampleMotionTimeline(timeline, 0.5)).toEqual([0.5, 0.125, 0, 0]);
});
