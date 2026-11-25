# Native household balance source

These are original decoded property records, not complete apartment Units.
Source: `SimCityDLCEP1-Scripts_287520926.package`, SHA-256
`0a3f66804d9a99a626509b42c76d3c308a53f9fb2d84529f621a88bf85043ff9`.
All source bytes are hash-pinned by `behavior_copan_residence.rs`.
Their six native bin declarations are also registered in
`../behavior-bin-catalog.json`; existing catalog entries remain unchanged.

| File | Native source TGI / property |
| --- | --- |
| 78a730cd.bin | 00B1B104:61EFC700:78A730CD |
| 78a730ce.bin | 00B1B104:61EFC700:78A730CE |
| 78a730cf.bin | 00B1B104:61EFC700:78A730CF |
| ebfc1753.bin | 00B1B104:61EFC700:EBFC1753 |
| ebfc1750.bin | 00B1B104:61EFC700:EBFC1750 |
| ebfc1751.bin | 00B1B104:61EFC700:EBFC1751 |
| cleanup-input.bin | 00B1B104:61EFC000:93E3DB12, 0E816C38 |

The three 78A730C* records remain native references, with no copied resource
emitted. One owned bundle per household Unit derives from its EBFC175*
record. It removes only the six explicitly listed floor finance, tower park
bonus, floor registration/cleanup roots, and their two context-only bin rows.
The nine remaining bin rows, source event/sink rules and whole-toggle player
callbacks (0C614160) remain original. No native AEB rule library is rewritten.

The common household cores and wealth-specific authored inputs still use
the existing ground residence compiler. This adapter replaces its competing
intake references; it does not copy floor identities, tower ownership,
construction, pricing, menu, geometry or floor registration properties.
0E816C38=100 is the native household cleanup input, not an occupancy counter.

See `docs/re/plop-economic-abandonment-contract-20261125.md` for boundaries
and `tmp/residence-minimal-loop-20261125/apartment-balance-boundary.json`
for source dependency analysis. Static recipe verification does not prove
actual movement, admission, utility restoration or night lighting in-game.
