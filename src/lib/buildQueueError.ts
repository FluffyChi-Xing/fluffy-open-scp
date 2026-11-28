export function buildQueueErrorKey(error: string): string | undefined {
  if (/\broad_mesh_raster_invalid\b/.test(error)) {
    return "roadAsset.project.meshRasterInvalid";
  }
  if (/\broad_piece_variant_missing\b/.test(error)) {
    return "roadAsset.project.pieceVariantMissing";
  }
  switch (error) {
    case "BUILD_QUEUE_TIMEOUT":
      return "studio.assetWorkbench.buildQueueTimeout";
    case "BUILD_QUEUE_INTERRUPTED":
      return "studio.assetWorkbench.buildQueueInterrupted";
  }
}
