export function buildQueueErrorKey(error: string): string | undefined {
  switch (error) {
    case "BUILD_QUEUE_TIMEOUT":
      return "studio.assetWorkbench.buildQueueTimeout";
    case "BUILD_QUEUE_INTERRUPTED":
      return "studio.assetWorkbench.buildQueueInterrupted";
  }
}
