# The GlassBox Engine — A Primer

GlassBox is the simulation engine of SimCity (2013). It is a
**data-driven** architecture: the engine is a generic interpreter and
scheduler; behavioral data (rules, resource tables, agent parameters)
lives entirely on the `.package` data side.

## Layered structure

```
GB namespace (85 classes)   Engine runtime: game manager, state machines,
                            delta sync, physics world
SC namespace (325 classes)  Game simulation: agents, transport pipes,
                            rendering constants
Swarm (EA)                  Particle & swarm systems (peds/traffic)
Bullet                      Physics middleware (presentation only:
                            debris, ground raycasts, separation)
Wwise                       Audio middleware
```

## Core simulation mechanics

- **Transport pipes**: roads are "pipes"; vehicles and pedestrians are
  agents flowing through pipe slots; congestion is tiered by occupancy
  (distributed LOD scheduling).
- **Thin agents / heavy pools**: an agent is just a row of data (sentinel
  ids + a few params); position and state live in pools and grids.
- **Fields + flows economy**: resources are grid scalar fields, carried
  over time by Swarm streams.
- **Data-driven rules**: `cBeatRuleHandler` drives rule tables on beats;
  the event queue doubles as a backpressure valve.

## Why this matters for modding

- Rebalancing game behavior means editing property tables in packages —
  the engine never changes;
- This tool (OpenSCP) exploits exactly that: property editing and overlay
  write-back change simulation purely from the data side.
- The engine is a single 32-bit exe; rule hashes live in data tables, not
  instruction immediates — offline analysis requires squeezing from both
  the RTTI side and the data-table side.

> Full analyses: `docs/overview/glassbox-engine.md` and
> `docs/overview/glassbox-subsystems-deep.md`.
