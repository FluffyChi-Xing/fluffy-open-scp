use std::collections::HashMap;

use serde_json::{json, Value};

const CLOCK_MAX_AGE_MS: u64 = 5_000;
const STALL_MS: i64 = 3_000;
const SPEED_CHANGED: u64 = 182_429_322;

#[derive(Clone, Copy)]
struct ClockSample {
    time: i64,
    seconds: f64,
}

#[derive(Default)]
pub(super) struct BridgeTime {
    samples: Vec<ClockSample>,
    by_page: HashMap<String, Vec<ClockSample>>,
    latest_speed: Option<(i64, bool)>,
    last_seen: i64,
}

fn page(row: &Value) -> Option<&str> {
    row["page"].as_str().or_else(|| row["binding"].as_str())
}

fn paused(speed: &Value) -> Option<bool> {
    match speed.as_u64().or_else(|| {
        speed
            .as_f64()
            .filter(|v| v.fract() == 0.0)
            .map(|v| v as u64)
    })? {
        182_362_487 => Some(true),
        182_362_527 | 182_362_531 | 182_362_534 => Some(false),
        _ => None,
    }
}

impl BridgeTime {
    pub(super) fn observe(&mut self, row: &Value, calls: &mut HashMap<i64, (String, String)>) {
        let Some(time) = row["time"].as_i64() else {
            return;
        };
        self.last_seen = self.last_seen.max(time);
        if row["eventID"].as_u64() == Some(SPEED_CHANGED) {
            if let Some(value) = row["event"]["items"][1]["properties"]
                .as_array()
                .and_then(|properties| {
                    properties
                        .iter()
                        .find(|property| property["name"] == "speed")
                })
                .and_then(|property| paused(&property["value"]["value"]))
            {
                self.observe_speed(time, value);
            }
        }
        let Some(binding) = row["binding"].as_str() else {
            return;
        };
        match row["channel"].as_str() {
            Some("invoke") => {
                if let (Some(sequence), Some(request @ ("periodTimes/0" | "gamespeed"))) = (
                    row["sequence"].as_i64(),
                    row["arguments"][0]["text"].as_str(),
                ) {
                    calls.insert(sequence, (binding.into(), request.into()));
                }
            }
            Some("result") => {
                let Some((caller, request)) = row["callSequence"]
                    .as_i64()
                    .and_then(|id| calls.remove(&id))
                else {
                    return;
                };
                if caller != binding || row["success"].as_bool() == Some(false) {
                    return;
                }
                let result = &row["result"]["value"];
                if request == "gamespeed" {
                    if let Some(value) = paused(result) {
                        self.observe_speed(time, value);
                    }
                } else if let Some(fraction) = result.as_f64().filter(|v| (0.0..=1.0).contains(v)) {
                    let sample = ClockSample {
                        time,
                        seconds: fraction.rem_euclid(1.0) * 86_400.0,
                    };
                    self.samples.push(sample);
                    if let Some(page) = page(row) {
                        self.by_page.entry(page.into()).or_default().push(sample);
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn latest_clock(&self) -> Option<f64> {
        self.samples.last().map(|sample| sample.seconds)
    }

    pub(super) fn paused(&self) -> Option<bool> {
        let (time, paused) = self.latest_speed?;
        if paused
            && self.samples.last().is_some_and(|sample| sample.time > time)
            && self.advancing() == Some(true)
        {
            return None;
        }
        Some(paused)
    }

    fn observe_speed(&mut self, time: i64, paused: bool) {
        if self
            .latest_speed
            .is_none_or(|(previous, _)| time >= previous)
        {
            self.latest_speed = Some((time, paused));
        }
    }

    pub(super) fn advancing(&self) -> Option<bool> {
        let last = self.samples.last()?;
        if self.last_seen.abs_diff(last.time) > CLOCK_MAX_AGE_MS {
            return None;
        }
        let before = self
            .samples
            .iter()
            .rev()
            .skip(1)
            .find(|sample| sample.time < last.time)?;
        if last.time.abs_diff(before.time) <= CLOCK_MAX_AGE_MS && last.seconds != before.seconds {
            return Some(true);
        }
        let mut same_since = last.time;
        for sample in self.samples.iter().rev().skip(1) {
            if sample.seconds != last.seconds || same_since.abs_diff(sample.time) > CLOCK_MAX_AGE_MS
            {
                break;
            }
            same_since = sample.time;
        }
        (last.time.saturating_sub(same_since) >= STALL_MS).then_some(false)
    }

    pub(super) fn finish(&mut self) {
        self.samples.sort_by_key(|sample| sample.time);
        for samples in self.by_page.values_mut() {
            samples.sort_by_key(|sample| sample.time);
        }
    }

    pub(super) fn annotate(&self, row: &mut Value) {
        let Some(time) = row["time"].as_i64() else {
            return;
        };
        let sample = page(row)
            .and_then(|page| self.by_page.get(page))
            .and_then(|samples| nearest(samples, time))
            .or_else(|| nearest(&self.samples, time));
        if let Some(sample) = sample {
            row["gameClockSeconds"] = json!(sample.seconds);
            row["gameClockSource"] = json!("ui_bridge.periodTimes/0");
            row["gameClockAgeMs"] = json!(time - sample.time);
        }
    }
}

fn nearest(samples: &[ClockSample], time: i64) -> Option<ClockSample> {
    let index = samples.partition_point(|sample| sample.time <= time);
    index
        .checked_sub(1)
        .and_then(|i| samples.get(i))
        .into_iter()
        .chain(samples.get(index))
        .min_by_key(|sample| sample.time.abs_diff(time))
        .copied()
        .filter(|sample| sample.time.abs_diff(time) <= CLOCK_MAX_AGE_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe_clock(bridge: &mut BridgeTime, time: i64, fraction: f64) {
        let mut calls = HashMap::new();
        bridge.observe(
            &json!({
                "channel": "invoke", "binding": "0x11", "sequence": 1, "time": time,
                "arguments": [{"text": "periodTimes/0"}]
            }),
            &mut calls,
        );
        bridge.observe(
            &json!({
                "channel": "result", "binding": "0x11", "page": "0x22", "callSequence": 1,
                "time": time, "result": {"value": fraction}
            }),
            &mut calls,
        );
    }

    #[test]
    fn timestamps_rows_from_nearby_samples_across_bindings() {
        let mut bridge = BridgeTime::default();
        observe_clock(&mut bridge, 1000, 0.25);
        observe_clock(&mut bridge, 3000, 0.5);
        bridge.finish();

        let mut row = json!({"time": 2800, "binding": "0x33"});
        bridge.annotate(&mut row);
        assert_eq!(row["gameClockSeconds"], 43_200.0);
        assert_eq!(row["gameClockAgeMs"], -200);

        let mut stale = json!({"time": 9000});
        bridge.annotate(&mut stale);
        assert!(stale.get("gameClockSeconds").is_none());
        assert_eq!(bridge.advancing(), Some(true));
    }

    #[test]
    fn stationary_clock_is_distinct_from_a_reported_pause() {
        let mut bridge = BridgeTime::default();
        observe_clock(&mut bridge, 1000, 0.5);
        observe_clock(&mut bridge, 2500, 0.5);
        observe_clock(&mut bridge, 4200, 0.5);
        let mut calls = HashMap::new();
        bridge.observe(&json!({
            "time": 4200, "channel": "result", "requestName": "globalFlags", "result": {"value": true}
        }), &mut calls);
        bridge.finish();
        assert_eq!(bridge.advancing(), Some(false));
        assert_eq!(bridge.paused(), None);
        bridge.observe(&json!({
            "time": 4300, "eventID": SPEED_CHANGED,
            "event": {"items": [{}, {"properties": [{"name": "speed", "value": {"value": 182362487}}]}]}
        }), &mut calls);
        assert_eq!(bridge.paused(), Some(true));
    }
}
