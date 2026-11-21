/** Single-flight scene orchestration, independent of Vue and the assembly implementation. */
export function createSceneRebuildScheduler(options: {
  suspended: () => boolean;
  whenSettled: () => Promise<void>;
  rebuild: () => Promise<void>;
  requestFrame: (callback: () => void) => void;
  onError: (error: unknown) => void;
}) {
  let scheduled = false;
  let requested = false;
  let disposed = false;
  return {
    request() {
      if (disposed || options.suspended()) return;
      requested = true;
      if (scheduled) return;
      scheduled = true;
      options.requestFrame(() => {
        void (async () => {
          try {
            while (requested && !disposed && !options.suspended()) {
              await options.whenSettled();
              if (disposed || options.suspended()) break;
              requested = false;
              await options.rebuild();
            }
          } catch (error) {
            options.onError(error);
          } finally {
            scheduled = false;
          }
        })();
      });
    },
    dispose() {
      disposed = true;
      requested = false;
    },
  };
}
