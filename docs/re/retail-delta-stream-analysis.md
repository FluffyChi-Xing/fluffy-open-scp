# Retail Delta Stream Analysis

## 2026-10-17 correction: retail action semantics

The actual retail encoder at VA `0x5CC130` uses data2 extension/sentinel
`0xFE/0xFF`. Its decoder at `0x5CC1D0` accepts an additional version argument:
versions below 27 use legacy `0x3E/0x3F`; versions 27+ use `0xFE/0xFF`.
The outer MFS version 4 is not a verified value for this argument.
The offline header decoder now requires an explicit `stream_version`.
Retail action 2 selects the current unit; action 1 then carries data1=0 and
data2=stateIndex. The development enum's `Unused2` must not be applied here.
See [retail entrypoint evidence](retail-rule-logging-entrypoints-20261017.md).
Historical XOR byte candidates below remain unproven as action streams.

## Verified envelope

`SLDelta*_1.mfs` begins with a little endian `uint32` version (`4`). The
observed retail record layout is:

```text
ff 02 03 00
big endian uint32  record_size
big endian uint64  delta_id
big endian uint32  sim_ticks
big endian uint32  kind
byte[]              payload
```

The records are contiguous, and the payload length
is a multiple of 24 in the initial captured files. Delta IDs were monotonic in those samples only;
2026-10-17 live-save copies of SLDelta8_1 and SLDelta9_1 contain structurally valid
ID regressions (63528→63475 and 63529→63476). Parsers now preserve file order
rather than rejecting these files. The cause of the regressions remains unresolved.
See `evidence/delta-import-regressions-20261017.json` for hashes, offsets and counts.
Not every .mfs file is a standalone v4 envelope: the observed SLDelta11_1 header
is `24 7e 0d 00`, and SLDelta11_2 lacks the expected record marker at offset 4.
Their wrapper/segmentation semantics are not established by this inspection.
This is an outer retail envelope;
it must not be passed directly to the development `EncodeActionHeader`
decoder.

## Retail runtime evidence

For the observed executable (module base `0x400000`):

| Type | Runtime vftable |
| --- | ---: |
| `GB::cStreamXOR` (IStream view) | `0xd1ae54`, `0xd1ae5c` |
| `GB::cDeltaStream` | `0xd1aafc` |
| `GB::cMD5Stream` | `0xd1ad00` |

The `cStreamXOR` methods are stable in this build:

- read/decode: `0x5c6010` (vtable slot 12)
- write/encode: `0x5c6060` (vtable slot 14)
- key pointer: object `+0x0c`
- key length: object `+0x10`
- XOR phase: object `+0x14`

`scan_delta_xor.py` found no live `cStreamXOR` or `cDeltaStream` instance while
the game was at the main menu. A city load is required to capture the key or
the post-XOR stream.

During a live city interval with `SLDelta*.mfs` files changing, a bounded
Frida interceptor on `0x5c6010` (read) and `0x5c6060` (write) observed zero
calls and zero live objects. This indicates that the local save writer either
bypasses `cStreamXOR`, or creates and destroys these streams outside the
observation window. The interceptor is retained in
`tools/dynamic/capture_stream_xor.py` for the next load/save boundary.

## Successful live capture

After correcting the x86 `__thiscall` convention (`this` is `ECX`), the same
interceptor captured a live object:

- object: `0x6373fc58` (later short-lived objects had the same vftable)
- vftable: `0xd1ae5c`
- key address: `0x00d1add0`
- key length: `128` bytes
- captured key SHA-256: `4ff49592ade2576c322f3bab09954b926d19cc4b2d997621dc163eac233d826`

The hook captured 4/8-byte plaintext writes before XOR. The reconstructed
sample begins with repeated state fields and contains valid action-header
candidates at offsets 53, 189, 325, 461, and 621; these candidates repeat at
136-byte intervals. The raw samples and key are in
`tmp/dynamic/live/delta-xor/capture/`. This confirms that the key is usable for
stream-level reconstruction, although the captured window did not yet include
a complete road-placement action.

A second bounded capture collected 200 valid writes (832 plaintext bytes)
across eight short-lived objects sharing the same key. Decoding the
concatenated phase-ordered bytes produced action IDs `2, 3, 5, 6, 7, 8, 9,
10`; action ID `3` recurred with `data1=0xf900` and `data2=0xb1`. This is the
first direct evidence that the XOR hook is exposing structured delta actions,
rather than only container metadata.

The extended 60-second capture used separate read/write quotas and collected
3,689 valid writes (13,928 plaintext bytes) across 31 stream objects. It
yielded 192 action-header candidates spanning IDs `0..27`, including IDs 12,
16, 26, and 27 with non-zero parameters. This capture covers the requested
save, menu transition, reload, and subsequent city activity window.

Offline grouping shows that the 192 candidates are not one continuous global
action log. Five large objects each contain repeated groups such as
`26 -> 12 -> 27 -> 16 -> 22 -> 13`; several smaller objects contain one
state-specific ID. The repetition across objects means these are serialized
per-stream state records. The generated machine-readable report is
`tmp/dynamic/live/delta-xor/capture/analysis.json`; action names should only be
assigned after correlating a single object with a known user action.

## Boundary correction from the 65-second trace

The follow-up trace attached to the retail `GB::cDeltaStream` vtable while the
game was running. The only observed calls entered shared reference-counting
functions with objects whose vtables were `0xd1a0a8`, `0xd2ff30`, or
`0xd3ebd8`; none had the exact `cDeltaStream` vtable `0xd1aafc`. After strict
vtable filtering, the window contained zero `cDeltaStream` calls. The trace is
in `tmp/dynamic/live/delta-stream/calls.jsonl` and the tracer is
`tools/dynamic/capture_delta_stream.py`.

A later 120-second run did observe the exact `cDeltaStream` vtable. It created
34 short-lived objects, with internal fields consistent with delta IDs 7 through
23 and a 1536-byte write chunk. The only observed virtual calls were slots
0/1/2 (destruction and reference lifetime); no payload read/write slot was
entered. This identifies the manager lifetime but still does not expose the
action bytes. The next capture records each run separately and includes the
caller return address so the internal child stream can be located.

The layer trace resolved the actual write path: retail
`cMultiFileStreamWrapper::Write` is `0x5c1f70`, while the XOR pre-write entry is
`cStreamXOR::Write` at `0x5c6060`. Wrapper writes reproduce the outer
`ff 02 03 00` record envelope, usually one byte at a time. XOR writes are the
inner state stream and are mostly 4-byte or 8-byte fields. The standalone
tracer is `tools/dynamic/capture_delta_layers.py`; each run is stored under
`tmp/dynamic/live/delta-layers/`. The current differential window showed no
new non-periodic action header, so the next comparison must include one
explicit road or building placement during the capture.

This explains why the XOR capture contains repeated serializer markers and
why apparent action headers such as IDs 16 and 27 have impossible parameters.
Those bytes are fields in per-object state snapshots, not a proven action
sequence. A valid action decode requires capturing the stream that receives
`GB::cEcoGame::LogAction` output, or observing a `cDeltaStream` instance at the
save/load boundary; the current evidence does not establish either path.

## Interpretation boundary

The payload prefix `00 00 01 a1` and the repeated high-entropy bytes are
observations only. They are not currently identified as an action header,
compression marker, or business kind. The parser therefore preserves payload
bytes and does not guess their meaning.

## Frontend placement boundary (2026-10-13)

Road and building placement must be attributed to the EAWebKit UI command
bridge before interpreting any delta bytes. A 180-second passive native trace
is preserved at `tmp/ui-native-road-building-20261013/trace.jsonl` (summary:
`status.json`). It retained 333 command invocations and 4,373 direct UI event
calls without changing the game process.

The command stream contains the tool-side inputs, including `RoadDrawType/1`,
`selectTool/<hashed asset id>`, `setCursor/<x>,<y>,<z>`, and `setCursor/`.
The reverse calls identify `simcity.kGameMessagePlopToolSelected`
(`eventID=216227805`) and `simcity.kGameMessageUnitRuleUIEvent`
(`eventID=199450092`). The latter carries an array payload with an inner
`eventID`, `eventPosition`, and `unitRef`; it is the candidate channel for the
actual click/placement commit and should be retained as structured data.

This run did not produce a literal `Place` or `Build` `PostGameCommand`. That
does not mean that no building was placed: several tool-selection windows are
followed by `UnitRuleUIEvent` arrays whose inner `eventID`, `eventPosition`,
and changing `unitRef` identify interaction records that may contain the
placement commit. A successful placement must therefore be classified from
the event payload and its state transition, not from the presence of a
human-readable command name. The correct attribution is: frontend command
bridge for raw command strings; frontend event bridge for placement payloads;
engine `LogAction`/delta only as application-side confirmation; and
`SLDelta*.mfs` for save serialization.

The high-frequency `RequestGameData` traffic is sampled and is not a command
source. The trace summary reports 147,557 dropped query samples but zero
dropped command samples; this does not affect the retained command evidence.

## Full raw bridge capture (2026-10-14)

The earlier sampled trace is not sufficient for attribution. The observer was
updated with an explicit exhaustive mode that raises the per-channel queue,
batch, and disk limits. The resulting trace is
`tmp/ui-native-exhaustive-build-20261014/trace.jsonl`; its status reports
142,566 records with `dropped=0` and `decodeErrors=0`:

- 70,363 raw `RequestGameData` calls and 70,363 matched results;
- 1,557 `ReceiveGameEventFromClient` calls;
- 283 raw `PostGameCommand` calls.

Near the save boundary the raw sequence contains `populationSave/start`,
multiple `populationSave/<id>/-1`, and `populationSave/end`, while the same
timeline contains ordinary `RequestGameData` calls and their changing scalar
results. These are recorded as observations only. The capture does not assume
that a save, road placement, or building placement is represented by any one
bridge category. Any attribution must be derived from the complete ordered
records and checked against the corresponding delta bytes.

The synchronized raw trace gives concrete sequence anchors without assigning
meaning to a bridge category: `RoadDrawType/1` occurs at UI sequence `113020`
and is followed by `properties/...` queries with object and array results. The
save marker occupies sequences `163409` through `163421`; immediately after
it, the UI requests `units/2,0/propKey`, `units/3,0/propKey`, and subsequent
unit records. These are ordered observations of the data flow and are the
appropriate inputs for a later byte/state diff.

The paired delta file `tmp/dynamic/live/delta-layers/1791907559.jsonl`
contains 12,000 writes (9,807 wrapper writes and 2,193 XOR writes). That run
predates the per-write timestamp fix, so it can be aligned with the UI trace
only at the capture-window level; no exact UI-record-to-delta-write mapping is
claimed from it.
