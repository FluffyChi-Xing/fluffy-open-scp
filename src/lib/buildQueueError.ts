export function buildQueueErrorKey(error: string): string | undefined {
  if (/\broad_mesh_raster_invalid\b/.test(error)) {
    return "roadAsset.project.meshRasterInvalid";
  }
  switch (error) {
    case "BUILD_QUEUE_TIMEOUT":
      return "studio.assetWorkbench.buildQueueTimeout";
    case "BUILD_QUEUE_INTERRUPTED":
      return "studio.assetWorkbench.buildQueueInterrupted";
  }
}
