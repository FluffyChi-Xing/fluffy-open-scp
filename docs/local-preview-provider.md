# Installed-game preview provider

OpenSCP ships authored preview code; original game images, CSS, layouts and locale tables are read locally from the selected SimCity installation. No pre-extracted replica, frontend dump, or old application cache is a prerequisite.

## Setup

1. Open ModStudio's UI workbench in the desktop application.
2. Choose **Connect installed SimCity** and select its `SimCityData` directory.
3. Wait for package verification and model construction. Select an installed game-text language, then inspect categories and submenus.

Cancellation preserves the existing attachment. Language choices are limited to installed supported tables. If the preferred language is absent, the workbench explicitly reports the available language selected for this preview; unavailable choices remain disabled. Missing required resources produce diagnostics rendered as text. Reconnect after changing game packages. The provider does not download resources, evaluate game scripts, modify game files, or export resources into mods. Browser-only mode has no native filesystem provider.

## Identity and resource handling

Every read is bound to an immutable installation generation. The native provider constructs JSON from original package resources, and returns exact type/group/instance image references. The frontend converts only allowlisted image media into data URLs. Original CSS identities come from the installed package inventory; authored layout primitives and calibration styles are maintained separately under `src/lib/game-ui/replica/authored`, with upstream provenance notices.

CSS cannot import remote styles, execute expressions or use escaped/obfuscated URL syntax. Missing images are reported explicitly according to the structural/degraded-control policy below. Legacy nine-slice declarations are filtered to classes referenced by the model before their fragment resources are requested. A stylesheet inventory containing unused fragment names is not evidence of the same number of broken visible images. JSON is data only; the application does not execute imported game JavaScript or HTML.

The provider boundary rejects arbitrary filesystem paths and external network resources. It is not a process sandbox against a malicious application running as the same OS user. Generated diagnostics and local application state must not be uploaded as redistribution packages. Preserve original investigation output separately; this provider does not import or delete it.

## Validation scope

Unit checks establish generation propagation, installation-only setup and rejection of active CSS. Actual desktop validation must separately record a clean preparation with the old extraction/cache unavailable, installed-language switching, category/submenu interaction, and required-resource failures. Type/build success alone does not certify rendered game fidelity, native runtime, terrain rendering, or release packaging.

## Static authoring limits

The compatibility adapter preserves the upstream authoring scene and calibration: the original 14 categories, 14 shared-HUD branches, category cells of 43x62, category pitch 46.85 and palette pitch 121.6. It resolves the original module resources by verified type/group/instance references and retains the upstream instance namespaces. This does not modify installed layouts or reproduce live controller visibility.

The preview reconstructs declarative installed layouts and properties; it does not run the game controller or simulate unlock conditions. The existing static locked badges are preserved as authoring presentation; actual runtime unlock status remains unknown. Geometry preserves upstream editor calibration rather than claiming verified runtime geometry. Missing alternate-state images are reported and the renderer may use the normal state, matching the existing renderer policy. Missing normal backgrounds are marked as degraded, without invented bytes; structural failures stop model construction. Source resource identities remain metadata and are never requested as image payloads.

Original stylesheet order and generated HUD transformations are preserved. The original game CSS is read from the installation; only the separately attributed authored reconstruction rules ship with OpenSCP. Park entries are reconstructed through their installed property references instead of relying on the old bundled workbench JSON. Traditional Chinese (`zh-TW`) remains a distinct supported source locale and is not coerced to Simplified Chinese.

The upstream adapter initially requests `zh-TW`, matching the original authoring corpus. If it is unavailable, the UI explicitly identifies the installed language selected instead; it does not present that fallback as Traditional-Chinese parity.

Source contents depend on the installed version and language. A comparison against a bundled Traditional-Chinese baseline using only an installed Polish locale cannot establish identical text or text-derived output. Functional regression and cross-locale differences must be reported separately; this is not permission to mark an unequal differential comparison as passing.

Installed preview images keep their provenance when represented as in-memory data URLs. Unchanged source images are not authored crop payloads and are omitted from overlay/package media output, including copies onto another item. Explicit authored image edits remain exportable. Passive rollover formatting such as bold, line breaks, lists and literal colors is retained through inert DOM reconstruction; active HTML, event handlers and resource URLs are excluded. Static HUD locale text stays raw because the existing renderer inserts it with `textContent`, preserving its original inert display semantics.

The original authoring corpus contains no type-1 nine-slice nodes. This source-only change retains the existing renderer rather than adding another drawable implementation. Original generated CSS rules are filtered before resource resolution, so unused fragment declarations do not trigger thousands of speculative reads. Initial loading can take time while package identities and images are verified; the previous model is cleared during replacement so failures cannot leave stale interactive controls.
