# Plop host utility ownership, 2026-11-25

## Reproduced defect

Deployed Peace Hotel SHA-256: `941941986209ad213cb9471172ce88f3c052aae5f556f53b19cd03c14f3df151`.
The host and four business components each enabled native external consumers.
A partitioned total budget preserved property sums but did not provide a single
building service boundary. A connected secondary unit is not automatically a
shared local bin set: `ConnectUnits`, retained decompile lines 634933–635090.
The existing paused snapshot is sufficient; no further runtime capture is needed.

## Native comparison and implemented boundary

Copan `40E0C000:4DC31F9B` combines its residential and facility services on one
Unit. The extracted common plop prototype is hash pinned in
`behavior_copan_base.rs`. MegaTower additionally uses internal service/floor
controllers; those controllers cannot be replaced by arbitrary independent
road consumers.

The host retains the entire authored/default power and water budget and native
`48939D96` / `825B4CA4` consumers. Components exclude these and their alternate
providers `19E5DB54` / `D6068BEA`, and remove all eight demand, storage and period
inputs. No additional utility inventories or global consumer counters are emitted.

Components use native connected-host activation rule `4D20B4A5`, reached in
native Copan bundle `472644E9` from root `0ECCE71E`. It tests the connected
host's `7A19A243` ActiveBuilding resource (not a city-wide active counter).
Retail rule source SHA-256:
`a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`.
The native ActiveBuilding bin row has capacity 1 and initial amount 0, as in
`30645625`. This state bin is not a utility buffer or a manufactured supply value.
The adapter is a new, minimal property bundle referencing existing native rules;
it is not an unchanged copy of an entire original Unit or an entire tower floor.

Engine evidence: connected-vs-nearby selection at decompile lines 698705–698742;
resource predicates at 701131–701201. Rule `4D20B4A5` succeeds through
`E0E6113A` / `8C58034F` / `607318D1`; its failed connected-host test runs
`9C5BD462` / `5BF091D6` to deactivate the component. Missing/closed hosts fail
this dependency. Local facility self-activation `30645625` is excluded only on
the invisible components. The visible host keeps its verified `30645625` wiring.

This is host-owned fault state with connected business activation, not a second
water/power consumer on every leaf. Components do not publish their own utility
fault or consumer statistics. Existing household migration and tourism bundles
are retained. Native household lifecycle `933F0FBF` is retained.

## Build and acceptance gates

The whole budget must remain on the host. A reintroduced component provider or
input, missing/modified connected-host bundle, duplicate adapter, missing child
connection, or failure to retain the adapter through identity closure fails the
build. Scene output cache version is bumped to v15 so old split-consumer results
cannot be reused. No custom binary rule library or stock rules override is emitted.

Static verification and runtime acceptance are separate. Production build,
resource comparisons and tests must be recorded in the release directory. Cold
start, utility flow, residence, tourism and night rendering remain user-run game
acceptance until actually confirmed. Only the user may launch SimCity.

## Workspace cleanup

167 obsolete `.build` cache directories removed: 62,878,944,464 logical bytes
(58.56 GiB). Small reports and ledgers retained in
`tmp/cleanup-retained-ledgers-20261125`. Source, current release inputs, GOLDEN
archives, saved cities and the existing full-city evidence were preserved.

42 obsolete copied test executables additionally removed: 953,203,712 bytes.
Total generated data removed: 63,832,148,176 bytes (59.45 GiB), before
retained ledger metadata and the small new release.

Production package: `39eb843fec2b222243c997043ce23cda2a19d87e3dbcd2939aaa5dd70188be92`,
new menu `A8F2B1C6`, deployed through the archive tool while the game was closed.
The existing shared panel runtime and all eleven retained packages are unchanged.
Verification: `tmp/peace-utility-v8-20261125/verification.json`; deployment ledger:
`tmp/peace-utility-v8-20261125/deployment-result.json`. Runtime acceptance pending.
