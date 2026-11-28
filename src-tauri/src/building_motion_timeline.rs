use sc_properties::{PropertyFile, Value};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionTrack {
    pub(crate) slot: u8,
    start: f32,
    end: f32,
    from: f32,
    to: f32,
    looping: bool,
    ping_pong: bool,
}

#[derive(Clone, Serialize)]
pub(crate) struct MotionTimeline {
    pub(crate) duration: f32,
    pub(crate) tracks: Vec<MotionTrack>,
}

/// FillFromProps reads parallel columns. GetAnimationRange takes the end time
/// of the sequence's first row, including a row without a model channel.
pub(crate) fn motion_timeline(
    props: &PropertyFile,
    id: u32,
    resolve_slot: impl Fn(u32) -> Option<u8>,
) -> Option<MotionTimeline> {
    let column = |hash| props.get(hash).and_then(|p| p.array()).unwrap_or_default();
    let key = |hash, i| match column(hash).get(i) {
        Some(Value::Key(key)) => Some(key.instance),
        _ => None,
    };
    let number = |hash, i| match column(hash).get(i) {
        Some(Value::Float(value)) if value.is_finite() => Some(*value),
        _ => None,
    };
    let flag = |hash, i| matches!(column(hash).get(i), Some(Value::Bool(true)));
    let rows = (0..column(0x0DA7_CAD5).len()).filter(|&i| key(0x0DA7_CAD5, i) == Some(id));
    let duration = number(0x0DA7_CAD7, rows.clone().next()?)?;
    if duration <= 0.0 {
        return None;
    }
    let tracks = rows
        .filter_map(|i| {
            let types = column(0x0DA7_CB06);
            if !types.is_empty()
                && !matches!(types.get(i), Some(Value::Int32(0) | Value::UInt32(0)))
            {
                return None;
            }
            let channel = key(0x0DA7_CAD4, i)?;
            if channel == 0 || channel == 748895195 {
                return None;
            }
            let start = number(0x0DA7_CAD6, i)?;
            let end = number(0x0DA7_CAD7, i)?;
            if end <= start || start >= duration {
                return None;
            }
            Some(MotionTrack {
                slot: resolve_slot(channel)?,
                start,
                end,
                from: number(0x0DA7_CAD8, i)?,
                to: number(0x0DA7_CAD9, i)?,
                looping: flag(0x0DAE_72FB, i),
                ping_pong: flag(0x0DAE_72FC, i),
            })
        })
        .collect::<Vec<_>>();
    (!tracks.is_empty()).then_some(MotionTimeline { duration, tracks })
}
