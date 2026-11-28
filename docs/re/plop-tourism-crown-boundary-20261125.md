# Continuous plop tourism: native crown business boundary

## Frozen, user-confirmed baseline

`000_OpenSCP_PeaceHotel_Utilities_20261125.package`, SHA-256
`39eb843fec2b222243c997043ce23cda2a19d87e3dbcd2939aaa5dd70188be92`,
menu `A8F2B1C6`. The user confirmed rendering, external utility lines,
residents, garbage, no false water/power complaints, the authored initial
happiness of 700, and panel coexistence on 2026-11-25. Tourism remained 0/450.
The package is registered as a scoped GOLDEN reference. Its immutable delivery
must be used for comparisons, never an overwritten `.build` path.

Future recipes inherit host-owned facility services and connected component
activation. Future custom panels inherit the shared, conflict-checked runtime.
Removing a facility provider also requires checking every business rule input
that provider supplied; absence of duplicate service consumers is insufficient.

## Reproduced missing business input

The baseline tourism component `58D8EB27` enables Tokyo Tower business bundles
`E15369A2` and `75D22FD2/1/0`. Native periodic souvenir replenishment in the
75D family tests `4B16C93D` (opening-hours mode). No active bundle in this
component supplies that bin after `30645625` was excluded to remove independent
facility ownership. Souvenirs start at zero. The unchanged native arrival
handlers `64E8EB52/51/50` require one souvenir and an OpenShoppers slot for each
arriving tourist. Thus the advertised capacity 450 does not prove the component
is able to receive visitors.

There is also a scheduling difference: `AE3774FA` publishes attraction at an
hour-clock trigger (`D5A08F93`); 75D replenishment uses day-clock opening times
(`12952639`). This is distinct from the missing-bin failure above.

Evidence: `tmp/peace-tourism-20261125/missing-opening-hours-evidence.json` and
`native-tourism-difference.json`, derived from existing captures/native rules.
No additional game memory or city data was collected.

## Minimal native replacement

Native MegaTower crowns `4EDC368A` / `869CB5E8` use the business-only bundle
`61EFC700:02E94AD8`. Its exact property payload is pinned at
`src-tauri/resources/sc5/tourism-crown/02e94ad8.bin`, SHA-256
`025f3c54c1ccbb31f2638288f1aed59e873fb233f9292ba2fffc345b955ba836`.
It has 21 business bins, nine arrival/return handlers, five native player
callbacks, the existing removal cleanup `6EC94BB9`, and the native periodic
root `ECFC5DF8`. No tower Unit, finance bundle `51E4FF80`, floor registration,
physical service consumer or new binary rule library is copied.

`ECFC5DF8` checks Normal, OpenForBusiness and ActiveBuilding, then replenishes
souvenirs and publishes attraction/wealth quotas every 30 simulation ticks.
It has no opening-hours bin or clock trigger. Failure runs `C2772ED8`, which
removes the published attraction/quotas and clears souvenir stock. The native
arrival handlers remain `64E8EB52/51/50`.

Pure Copan plop tourism now selects this bundle and removes the old four
publishers/schedules. Existing authored attraction, quotas, shopper and souvenir
capacity inputs are retained. Existing legacy unscoped and retail/hotel mixed
routes are unchanged. The visible host, its `30645625` lighting controller,
residence migration, external utilities, garbage, paths and shared panel runtime
retain the frozen contracts.

The build rejects a missing crown publisher, any enabled old daily publisher,
or changed authored attraction/quota/pool values. The component-table verifier
runs these gates again on the complete and written resource set. Cache output
version advances to v17.

## Verification boundary

Offline execution of the native first refresh publishes attraction 700 and
wealth quotas 200/150/100, fills the three souvenir bins, and leaves tourist
counts at zero. All three real arrival handlers then have sufficient local
resources. No tourist count is prefilled. Evidence:
`tmp/peace-tourism-20261125/first-refresh-execution.json`.

Thirty ticks bound the native publication/replenishment interval after the
component becomes active; they do not promise an actual visitor at frame zero.
Visitors still have to originate and travel through the existing native network.
Production-package comparisons and user runtime acceptance must be recorded
separately. Never change the live package set while the game is running.

## Production verification and replacement deployment

28 tests passed, including the real model written-package fixture and a test
requiring crown and legacy tourism to emit byte-identical car path bindings.
The native crown was added to the vehicle-arrival capability registry; the crown
verifier rejects a missing receiver mapping, preventing a new route from silently
skipping the existing vehicle path contract.

Package SHA-256:
`87a9a8b67156bcc43b5349b7c37be230a6adc0bacb016cf713ec12ddaa3ec5ac`,
new menu `E9678ECB`. Compared with the hash-pinned GOLDEN, the host Unit records,
all non-tourism business Unit records, all component Assets and vehicle path
bindings, connected controllers, household balance payload, model/material/texture
and Graphics payloads are unchanged after required identity rebinding. Tourism
Unit drift is exactly the enable-bundle selection column. The shared panel
runtime is byte-identical and all eleven retained custom packages are unchanged.

The previous utility package was removed from SimCityData and registered in the
external archive; only the latest tourism package remains installed. No game was
launched and no saved city was modified. Runtime tourist arrival is pending user
acceptance. Evidence: `tmp/peace-tourism-20261125/verification.json` and
`deployment-result.json`.

## User acceptance: frozen residential and tourism recipe

The user confirmed on 2026-11-25 that real tourists appeared immediately and
the residence/tourism combination operates correctly; the screenshot shows 200
low-wealth visitors. Package `87a9a8b67156bcc43b5349b7c37be230a6adc0bacb016cf713ec12ddaa3ec5ac`
is now the scoped GOLDEN runtime baseline for this completed combination.

User directive: single residential behavior, single tourism behavior and their
combination use this exact same recipe. The ordinary-single selection path in
`behavior_mega_tower::Selection::compile_with_copan_base(menu.is_some())` and the
behavior-leaf path in `plan.rs` both route plop buildings to
`behavior_copan_base::compile_building`. That function uses the same residential
balance adapter and native crown tourism configuration for single leaves and
combinations. These paths were inspected; the production regression matrix
already includes single behaviors and combinations. No alternate recipe is
introduced, and no rebuild or live package change was needed to record acceptance.

Single configurations are the confirmed design contract and share the tested
compiler path; they are not falsely recorded as separate in-game sessions.
