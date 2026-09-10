# SimCity File Types — A Primer

All game data in SimCity (2013) ships in `.package` files (DBPF containers).
Every resource is identified by a **TGI** triple (Type / Group / Instance).
These are the types modders touch most often.

## Common types

| Type ID | Name | Purpose |
|---|---|---|
| `0x00B1B104` | Property | Property tables: game vars, building params, lot definitions |
| `0x2F4E681B` | RW4 | Model/texture container (RenderWare4): meshes, textures, materials |
| `0x2F4E681C` | Raster | Raw pixel images: LotMask ground masks and more |
| `0x0A98EAF0` | Locale JSON | Localization string tables (UTF-8 BOM + JSON) |
| `0x0D9E5710` | Wwise audio | Sound effects / music (Wwise format) |
| `0x0A4D8D09` | Wwise Bank | Audio banks |
| `0x376840D7` | VP60 video | Cutscene videos (VP6 codec) |
| `0x2F7D0004/06/07` | PNG/TGA/GIF | Standard bitmaps |
| `0x03E421EC/ED/F0` | Greyscale Map | 8/32/16-bit grey/color field maps (terrain) |
| `0x276CA4B9` | TrueType Font | Game fonts |
| `0x0469A3F7` | Shader source | HLSL shader fragments |
| `0x02393756` | Cursor | Cursors (ICO/CUR containers) |

## Key concepts

- **TGI addressing**: references between resources (e.g. a lot pointing at a
  model) are stored as Key properties and resolve across packages.
- **Property tables** are the heart of the data-driven design — the engine is
  an interpreter; behavior semantics live entirely on the data side.
- **The lot quartet**: `LotSize`, `LotMask` (4-channel ground mask),
  `LotPlacementTransform` (model↔lot placement), and `LotColor1-4`
  (channel tint + alpha = ground texture index).
- **Material slots (Material Set)**: an RW4 material carries up to six
  slots — slot0 params, slot1 color control, slot2 normal, slot3 shader map,
  slot4 palette, slot5 interior map.

> For deep technical detail see `docs/overview/file-formats.md` in the repo.
