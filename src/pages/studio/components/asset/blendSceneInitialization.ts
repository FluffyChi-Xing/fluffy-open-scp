/** Runs only after model loading succeeds. Failures never roll back the model. */
export interface SceneInitializationStep {
  id: string;
  run: () => Promise<void>;
}

export interface SceneInitializationFailure {
  step: string;
  cause: unknown;
}

export async function initializeBlendScene(
  steps: readonly SceneInitializationStep[],
): Promise<SceneInitializationFailure[]> {
  const failures: SceneInitializationFailure[] = [];
  for (const step of steps) {
    try {
      await step.run();
    } catch (cause) {
      failures.push({ step: step.id, cause });
    }
  }
  return failures;
}
