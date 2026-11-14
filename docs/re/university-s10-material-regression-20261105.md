# University s10 material regression (2026-11-05)

## Incident

s10 added vehicle prop bin entries and changed trees/lights. The deployed
package rendered the building as flat colour and was archived from
SimCityData to D:/ea-games/simcity_offline/test-archive/.

The existing s10 wire test passed because it checked prop/light property
columns and counted generated material resources. It did not check the
material binding contract used by the compiled building RW4.

## Root cause

s9's building input was a single atlas material: 74 meshes, one material,
128 parameter columns, and the six atlas textures. s10 switched the model
input to 74 independent material groups with one parameter column each.
building40 still rewrote the native RW4 to the first material's six texture
instances. The other 73 groups were emitted as resources but were not bound
to the model. The package was therefore structurally readable while the game
could not resolve the expected texture rows.

This was a processor contract violation. Tree, vehicle, and light changes
were unrelated to the material loss.

## Fix and guard

insert_scene_model now rejects a building40 LOTM unless it has exactly one
atlas material, with an error naming the responsible processor and the
required atlas preparation step. A future multi-material path must normalize
the source through the atlas processor before it reaches the native model
compiler. A resource count or DBPF readback cannot substitute for this
semantic check.

The s11 build uses the s9 atlas model as its material baseline and retains
the s10 scene/property changes. Its report records the stage events and
package hashes for later comparison.

## Required regression gates

Every building material build must record: source model hash, material group
count, parameter-column count, six slot payload hashes, mesh-to-material
mapping, and the RW4 material slot instances. The build must fail before
publishing when these values violate the selected model profile. Scene-only
changes may proceed only when the material fingerprint matches the last
verified baseline or an explicit material change is declared.
