import { describe, expect, it } from "vitest";
import { Group, Matrix4, Quaternion, Vector3 } from "three";
import { actorHeadPosition } from "./actorHeadAttachment";
describe("native actor head attachment", () => {
  it("cancels the viewer Z-up conversion and actor scale without inheriting the bone rotation", () => {
    const world = new Group();
    world.rotation.x = -Math.PI / 2;
    const actor = new Group();
    actor.scale.setScalar(1.35);
    actor.rotation.z = 0.4;
    world.add(actor);
    const bone = new Group();
    bone.position.set(0.1, 0.02, 1.7);
    bone.rotation.x = Math.PI / 2;
    actor.add(bone);
    const head = new Group();
    actor.add(head);
    world.updateMatrixWorld(true);
    head.position.copy(actorHeadPosition(actor.matrixWorld, bone.matrixWorld));
    expect(head.position.distanceTo(new Vector3(0.1, 0.02, 1.7))).toBeLessThan(
      1e-12,
    );
    expect(head.quaternion.angleTo(new Quaternion())).toBe(0);
    world.updateMatrixWorld(true);
    expect(
      head
        .getWorldQuaternion(new Quaternion())
        .angleTo(actor.getWorldQuaternion(new Quaternion())),
    ).toBeLessThan(1e-7);
  });
  it("follows animated attachment positions while leaving the head upright in actor space", () => {
    const actor = new Matrix4()
      .makeRotationX(-Math.PI / 2)
      .scale(new Vector3(2, 2, 2));
    for (const z of [1.6, 1.7, 1.8]) {
      const bone = new Matrix4().makeRotationY(0.7).setPosition(0.1, 0, z);
      expect(
        actorHeadPosition(actor, actor.clone().multiply(bone)).distanceTo(
          new Vector3(0.1, 0, z),
        ),
      ).toBeLessThan(1e-12);
    }
  });
});
