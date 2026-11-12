# Blend import resources

OpenSCP runs its own Blender reader in factory-startup mode and generates LOTM
in Rust. Blender can be found on PATH or configured with `BLENDER_PATH`.

Before packaging from a checkout without the atlas, stage the data directory
that contains `catalog.json` and the pack folders:

```powershell
pwsh -File scripts/stage-blend-atlas.ps1 -Source 'D:/path/to/atlas/packs'
```

The installer includes `resources/blend-atlas`, which contains the catalog and
the five PNG slots for each pack. The optional `resources/blender/ny_facade`
directory is for offline reference comparisons and is outside the installer
resource list.
