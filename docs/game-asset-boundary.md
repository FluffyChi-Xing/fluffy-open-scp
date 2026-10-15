# Installed UI resource boundary

The UI preview reads original images, CSS, layout resources and localized text from the selected installed SimCityData directory. The historical `public/game-ui/` directory mixed extracted resources, generated game-derived data, reference screenshots and authored replica styles; its removal is a conservative distribution boundary, not a claim that every historical file had identical provenance.

The authored `replica.css` and `tweaks.css` are retained under `src/lib/game-ui/replica/authored`, with original comments and source attribution. The provider does not bundle or download original game UI payloads, evaluate game JavaScript, or require a pre-extracted frontend dump. See [setup and limitations](./local-preview-provider.md).

Vite rejects `public/game-ui/`, including directory junctions, before serving or building. Do not copy local extraction output into that directory. This change does not remove or classify the 16 existing `src/assets/ground` images: their immediate source was the SimCityPak source tree, and their independent provenance requires separate review. The guard intentionally covers only the UI resource boundary addressed here.

Deleting files at the branch tip does not remove them from Git history. Any historical rewriting or public redistribution review is a separate action; this change preserves the upstream update path and does not rewrite upstream commits.
