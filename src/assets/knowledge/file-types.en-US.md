# SimCity File Types — A Primer

All game data in SimCity (2013) ships in `.package` files (DBPF containers).
Every resource is identified by a **TGI** triple (Type / Group / Instance).
The table below lists every file type OpenSCP currently parses or previews,
together with its binary signature; chaptered details follow below.

## Parsed types — full table

| Type ID | Name | Binary signature | Parsing capability |
|---|---|---|---|
| `0x00B1B104` | Property | Binary property table (hash + type + value rows, no file magic); **sub-type = low 16 bits of GroupContainer (InstanceType)** | Full parse (incl. canonical write-back), lot/unit assembly |
| `0x2F4E681B` | RW4 | RenderWare4 container: header + section index + blobs | Mesh/material/texture/skeleton/anim decode, GLB/OBJ export |
| `0x2F4E681C` | Raster | Headerless raw pixels (LotMask = raw RGBA) | Decode + PNG preview |
| `0x2F7D0004` | PNG | `89 50 4E 47` | Direct preview |
| `0x2F7D0006` | TGA | TGA header (footer may contain `TRUEVISION-XFILE`) | Decoded to PNG preview |
| `0x2F7D0007` | GIF | `47 49 46 38` | Direct preview |
| `0x3F8662EA` | JPG | `FF D8 FF` | Direct preview |
| `0x02393756` | Cursor | ICO/CUR container (`00 00 01 00`) | Decoded to PNG preview |
| `0x03E421EC` | Terrain Field Map (8-bit) | 20-byte big-endian header `[0,w,h,1,byte_count]` + grey pixels | Decoded preview |
| `0x03E421ED` | Terrain Field Map (32-bit) | Same header, channel_code=2 — actually RGBA | Decoded preview |
| `0x03E421F0` | Terrain Heightmap (16-bit) | Same header, channel_code=7 — big-endian u16 single channel | Decoded preview |
| `0x0A98EAF0` | Locale JSON | UTF-8 BOM + JSON text | Text preview (BOM stripped) |
| `0x67771F5C` | JavaScript | Plain text | Text preview (syntax highlight) |
| `0x2C978DB6` | CSS | Plain text | Text preview |
| `0xDD6233D6` | HTML | Plain text (starts with `<`) | Text preview |
| `0x0469A3F7` | Shader source | HLSL plain text | Text preview (cpp highlight) |
| `0x024A0E52` | State Script | Plain text (`#` comments + `state`/`mode` keywords) | Text preview (ini highlight) |
| `0x0D9E5710` | Wwise audio | `OggS` / RIFF-WAVE (Wwise variants) | Playback preview (vgmstream) |
| `0x0A4D8D09` | Wwise Bank | `BKHD` magic | Structure detection + playback |
| `0x376840D7` | VP60 video | VP6 stream (FLV-family header) | Video preview |
| `0x276CA4B9` | TrueType Font | `00 01 00 00` (TTF) | Font preview |
| `0xEA5118B0` | Effects Directory | Binary effect directory table | Structure detection |
| `0x08068AEB` | ER2 Binary Rule | ER2 rule binary | Structure detection (unparsed) |
| `0x08068AAC` | ER2 Rule | ER2 rule text variant | Structure detection (unparsed) |
| `0x08068AED` | EcoGame State Data | `1F 8B` gzip; payload shares the offline-save .egb magic (`62 2b 9c d7` @+4) — EcoGame state snapshot | Hex preview (unparsed) |
| `0x08068AEE` | EcoGame State Table | 12-byte record table (sequence ids + bit-pattern fields) | Hex preview (unparsed) |
| — | DDS (embedded in RW4) | `44 44 53 20` ("DDS ") | DXT1/5 decode & export |
| — | SQLite registry | `53 51 4C 69 74 65` (database_main.s3db) | FileTypes/Instances queries |

## Chaptered details

### Property (0x00B1B104)

The heart of the data-driven design: the engine is an interpreter; behavior
semantics live entirely on the data side. Each property is a **hash
identifier + data type + value**; cross-resource references are stored as
**Key** (TGI triple) values and resolve across packages.

#### Sub-type discriminator: InstanceType = low 16 bits of GroupContainer

The property resource's **TGI type is always `0x00B1B104`**; the actual sub-type comes from
the **low 16 bits of the GroupContainer** — original SCP `PackageReader/DataBaseIndex.cs`:

```csharp
public uint InstanceType { get { return (_groupContainer & 0xffff); } } // mask 0000XXXX
```

The **high 16 bits are a volume/container index within the same sub-type** (a single lot can
appear as `0x40E1C000` and `0x42E1C000`, etc.) and play **no part** in sub-type discrimination.

| InstanceType | Sub-type | Notes |
|---|---|---|
| `0xC000` | **Unit** | Lot/building unit container: lights / effects / decals / props / paths / spawners **all live side-by-side as parallel columns inside one property** |
| `0xC600` | Agent | Sim (agent) definition |
| `0x8B7E` | Path | Path definition |
| `0xC400` | Network | Road network |
| `0xC900` / `0x8A01` | Menu / Menu2 | Menus |
| `0xE000` | Map | Map |
| `0x2043` | Descriptor | Descriptor |
| `0xB185` / `0x1651` / `0x1652` | DecalAtlas / DecalAtlas2 / DecalAtlas3 | Decal dictionary (gallery), **three volumes**; the only value this app special-cases (`is_decal_dictionary_group`) |

Verified: in `SimCity_Game.package`, lot `0xEE27D643` has group `0x42E1C000` → low 16 bits
`0xC000` = Unit ✓; its parent `0x40E1C000` likewise.

> ⚠️ **decal / prop / spawner are NOT property sub-types** — they are **unit kinds inside a
> Unit**, discriminated by which signature columns are present (below). Several kinds coexist
> in one Unit property: `0xEE27D643` carries prop columns (`0x0C12EF29`–`0x0C12EF2B`), decal
> columns (`0x0D109050`) and LOD references (`0x00F9EFBA`) at the same time.

#### Unit kinds: discriminated by their signature column

| Unit kind | Signature column (present ⇒ kind present) | Parallel columns |
|---|---|---|
| Light | `0x0CAA8F10` (LightIDs) | 14 |
| Effect | `0x02A907B5` (EffectIDs) | 5 |
| Decal | `0x0D109050` (Decal ID; categories 1/2 use `0x0D109051/52`) | 3 categories × 7 fields |
| Prop | `0x0C12EF40 + bin` (Prop Slots, bin 0–13) | 14 bins |
| PathPoint | `0x0CAA680D` (Path Points) | 4 |
| Spawner | `0x0E1BAC61` (Spawner IDs) | 5 |

#### The lot quartet and model references

| Identifier | Name | Type | Notes |
|---|---|---|---|
| `0x00F9EFBB`–`0x00F9EFBE` | UnitLOD1–4 | Key | Four model LOD references (point at RW4) |
| `0x0CCB7FC8` | LotSize | Float2 (vec2) | Ground size in meters |
| `0x0CCB7FC9` | LotOverlayOffset | Float2 | Ground overlay offset |
| `0x0CCB7FD5` | LotMask | Key | 4-channel ground mask (points at a Raster) |
| `0x0DB7FB17` | LotPlacementTransform | Transform (12 floats, row-major) | Model↔lot placement; ground uses its inverse |
| `0x0D02D586`–`0x0D02D589` | LotColor1–4 | Color values (RGBA) | Channel tint; A = ground texture index 0–15 |
| `0x0CCB7FD4` | Lot Textures | Key | Pure texture container (e.g. 1024² DXT5 ground map) |

#### Lights (Light units)

| Identifier | Name | Type |
|---|---|---|
| `0x0CAA8F10` | LightIDs | Int32 array |
| `0x0CAA8F11` | LightTypes | Key array (Point `0x75D4C8CD` / Spot `0x2F0FF9FD` / Line `0x0820ABAF`) |
| `0x0CAA8F12` | LightTransforms | Transform array (row-major 12 floats) |
| `0x0CAA8F13` | LightColors | Float3 array (RGB 0–1) |
| `0x0CAA8F14` / `0x0CAA8F15` | Radii / InnerRadii | Float array |
| `0x0CAA8F16` | DiffuseLevels | Float array |
| `0x0CAA8F17` | DebugNames | String array |
| `0x0D4C96B4` | SpecLevels | Float array |
| `0x0D4CAD23` | Lengths | Float array (Spot/Line) |
| `0x0E01200F` | CullDistances | Float array |
| `0x0E4DF244` | FalloffStarts | Float array |
| `0x0E98D445` / `0x0EC4567E` | IsVolumetric / VolStrengths | Bool / Float array |

#### Effects / decals / props / paths / spawners

| Identifier | Name | Type | Notes |
|---|---|---|---|
| `0x02A907B5`–`0x02A907BC` | Effect IDs/Transforms/AlwaysZero/RefIDs/Enabled | Key/Transform/Int32/Key/Bool arrays | Effect unit quintet |
| `0x0D109050`/`60`/`70`/`80` (+ category 0–2) | Decal ID/Transform/Depth/Material | Key/Transform/Float/binary | 3 categories × base offset. The unpacked exe's property registration (`FUN_0081d680`) declares three more: `0x0D109090` (RenderGroup, Key) / `0x0D1090A0` (MachineSpec, Int32) / `0x0D1090B0` (Float, rarely present) |
| `0x0C12EF2X` | ecoUnitBinDrawBinIDs | Key array | Prop prototype refs; **not resolvable offline** (compiled into internal game tables) |
| `0x0C12EF30` + bin | Prop Transforms | Transform array | 14 bins (0–13) |
| `0x0C12EF40` + bin | Prop Slots | Slot value array | Same |
| `0x0CAA680D` / `0x0CB00ED8` / `0x0CAA6832` | Path Points/Tangents/Indices | Float3/Float3/Int32 arrays | Path points |
| `0x0CAA6841` | PathPairs | Int32 pairs | Path ranges (semantics TBD) |
| `0x0E1BAC61` / `0x0E1BAC62` | Spawner IDs / Transforms | Key array (**element stride 0xC**) / Transform array (**stride 0x38**) | Spawner bodies, referencing an agent |
| `0x0E715928` / `0x0E715929` | Spawner count / randomization range | Int32 arrays | Spawn count; when the latter is non-zero, `count += random % latter` (proved in unpacked exe `FUN_00786000`) |
| `0x0F0E2BF1` | Spawner agent reference | Key | Resolved to an agent/sim through an engine manager (same proof) |

### RW4 (0x2F4E681B)

RenderWare4 container: header, section index, blobs, mesh/vertex
format/triangles/textures/bounding boxes. A material carries up to six
slots — slot0 params (f32), slot1 color control, slot2 normal, slot3 shader
map, slot4 palette, slot5 interior map. Textures are embedded DDS (DXT1/5).
Currently read-only; the writer is the first priority of the asset
development track.

### Raster and Terrain Maps (0x2F4E681C / 0x03E421EC-ED-F0)

Raster is headerless raw pixels (LotMask is raw RGBA, commonly 128²/256²).
The terrain family shares a 20-byte big-endian header
`[0, width, height, channel_code, byte_count]`: code 1 = 8-bit grey,
2 = RGBA, **7 = big-endian u16 single channel** (16-bit heightmaps,
verified on 256² samples).

### Text family (Locale JSON / JS / CSS / HTML / Shader / State Script)

Locale JSON carries a UTF-8 BOM (stripped in preview). State Script
(`0x024A0E52`) is the game state machine script: `state Main -id 0`,
`mode SimCity`, `cheat "..."` directives define state flow and startup
parameters.

### Audio/Video (Wwise / VP60)

Wwise audio (`0x0D9E5710`) uses OggS/RIFF variants; playback relies on
vgmstream. Banks (`0x0A4D8D09`) start with `BKHD`. VP60 (`0x376840D7`)
holds VP6-encoded cutscene videos.

### ER2 and EP1 variants (0x08068AEB–AEE)

The official ER2 Rule File types (`AEB/AC`) are rule-engine binary/text.
The EP1-only `AED` is a gzip (`1F 8B`) blob wrapping a 2.2 MB sparse hash
table, and `AEE` is a 12-byte record table — the type ids sit directly
adjacent to the ER2 series, so they are classified as EP1 variants;
structures remain unparsed and preview as hex for now.

> For deep technical detail see `docs/overview/file-formats.md` in the repo.
