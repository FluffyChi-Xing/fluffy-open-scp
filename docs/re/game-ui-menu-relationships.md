# Game UI menu relationships

The menu is assembled from several resources in the installed `SimCityData/*.package`
files. These resources serve different levels of the UI and must not be flattened into
one list by a matching category name.

| Resource | Role |
| --- | --- |
| `Layouts/GlobalUI2.js` (`0x67771F5C`, group `0x0B074E5A`) | City HUD and the city / region / big business mode graphics. The city category button container is populated at runtime. |
| `Layouts/Region/RegionUI.js` (`0x67771F5C`, group `0x0B074E5A`, instance `B717BCCF`) | Separate region HUD. Its button groups are great works, commuters, transport, utilities, services, and gifts. The package layout defines the containers; runtime code fills their buttons. |
| Menu/Menu2 property (`0x00B1B104`, instance types `0x8A01` and `0xC900`) | Tool records. `parent` identifies a menu parent, `uiCategories` supplies category membership, and `uiPosition` supplies order. |
| `Layouts/Palette/*` | Palette geometry, category buttons, tool slots, and rollover windows. A category's palette layout may differ from the general palette. |
| Locale tables and image resources | Tool names, descriptions, unlock text, slot icons, and rollover media. |

The extracted index is [tools.json](../../public/game-ui/tools/tools.json).
The currently rendered category list and palette layouts are in
[model.js](../../public/game-ui/replica/data/model.js) and
[palette.js](../../public/game-ui/replica/data/palette.js). The earlier
[workbench.json](../../public/game-ui/workbench.json) groups tools broadly by
category and loses the Menu/Menu2 `parent` relation. It must not be merged
directly into a visible palette: doing so places building submenu items next to
their parent building and produces extra placeholder icons. The city category
`3193C0BE` is zoning (residential, commercial, industrial, dezone); it is not
the separate region view.

The package also contains runtime-driven values. The static menu index does not
provide a verified mapping from every tool to its build cost or upkeep. The
workbench keeps those values unknown until a source is resolved or the editor
enters them.
