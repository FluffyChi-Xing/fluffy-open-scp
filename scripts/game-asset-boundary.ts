import { existsSync } from "node:fs";
import { resolve } from "node:path";

/**
 * @summary Reject game-derived preview data at the distribution boundary, including junctions.
 * @param root Repository root being served or built.
 * @throws Error when a forbidden public asset directory is present.
 */
export function assertNoBundledGameAssets(root: string): void {
  for (const path of ["public/game-ui"]) {
    if (existsSync(resolve(root, path))) {
      throw new Error(`Game assets must not be bundled: move ${path} outside the repository. See docs/game-asset-boundary.md.`);
    }
  }
}
