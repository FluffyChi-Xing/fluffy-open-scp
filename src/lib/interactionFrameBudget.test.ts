import { expect, it } from "vitest";
import { createInteractionFrameBudget } from "./interactionFrameBudget";

it("keeps normal motion and isolated spikes at their original ratio", () => {
  const budget = createInteractionFrameBudget();
  budget.begin(1);
  expect(budget.sample(3, 10000)).toBe(1);
  expect(budget.sample(4, 16.7)).toBe(1);
  expect(budget.sample(70, 70)).toBe(1);
  for (let i = 0; i < 20; i++) expect(budget.sample(4, 16.7)).toBe(1);
});

it("reduces sustained slow motion within bounds and resets on the next gesture", () => {
  const budget = createInteractionFrameBudget();
  budget.begin(1);
  for (let i = 0; i < 2; i++) expect(budget.sample(6, 30)).toBe(1);
  expect(budget.sample(6, 30)).toBe(0.8);
  for (let i = 0; i < 30; i++) budget.sample(6, 45);
  expect(budget.sample(6, 45)).toBe(0.5);
  // No oscillation while the pointer remains down.
  for (let i = 0; i < 20; i++) expect(budget.sample(4, 16)).toBe(0.5);
  budget.begin(0.5);
  for (let i = 0; i < 30; i++) expect(budget.sample(40, 45)).toBe(0.5);
  budget.begin(1);
  expect(budget.sample(4, 16)).toBe(1);
});

it("jumps directly to the minimum after sustained heavy GPU stalls", () => {
  const budget = createInteractionFrameBudget();
  budget.begin(1);
  budget.sample(6, 64);
  budget.sample(6, 64);
  expect(budget.sample(6, 64)).toBe(0.5);
});
