# Generate the small, identity-only catalog consumed by the independent scene adapter.
# Inputs remain the captured Packager catalog; no runtime plugin dependency.
param(
  [string]$Props = 'investigation/sc5-asset-packager-0.8.0/prop_assets/catalog.json',
  [string]$Effects = 'tools/sc5_asset_packager/runtime/effect_catalog.json',
  [string]$Output = 'src-tauri/src/blender_scripts/scene_catalog.json'
)
$propCatalog = Get-Content -LiteralPath $Props -Raw | ConvertFrom-Json
$effectCatalog = Get-Content -LiteralPath $Effects -Raw | ConvertFrom-Json
$propPresets = [ordered]@{}
foreach ($entry in ($propCatalog.presets.psobject.Properties | Sort-Object Name)) {
  $propPresets[$entry.Name] = [ordered]@{resourceId=$entry.Value.resourceId; preferredBin=$entry.Value.preferredBin}
}
$effectPresets = [ordered]@{}
foreach ($entry in ($effectCatalog.presets.psobject.Properties | Sort-Object Name)) {
  $effectPresets[$entry.Name] = [ordered]@{label=$entry.Value.label; effectId=$entry.Value.effectId}
}
[ordered]@{props=$propPresets; effects=$effectPresets} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Output -Encoding utf8
