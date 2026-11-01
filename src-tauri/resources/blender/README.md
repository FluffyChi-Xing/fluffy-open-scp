# Bundled Blender asset service

OpenSCP ships the SC5 Asset Editor exporter here so end users do not install a
Blender add-on manually.

Expected layout after staging:

```text
resources/blender/ny_facade/__init__.py
resources/blender/ny_facade/native_asset.py
resources/blender/ny_facade/assets/...
```

`blender_service::addon_parent()` looks for `ny_facade` under this directory.
For local development, set `OPENSCP_BLENDER_ADDON` to the add-on parent instead
of staging it.

Staging is an explicit build step (`scripts/stage-blender-addon.ps1`) because
the exporter and its atlas library are third-party assets. Do not publish a
build unless the exporter's redistribution terms are satisfied.
