import { cherryPickTargets, toMoveBranchWarning, type Stack } from "$lib/stacks/stack";
import { TestId } from "@gitbutler/ui";
import { describe, expect, test } from "vitest";

function stackWithBranches(...branchNames: (string | null)[]): Stack {
	return {
		id: "stack-1",
		base: null,
		segments: branchNames.map((name) => ({
			refName: name === null ? null : { displayName: name, fullNameBytes: [] },
		})),
	} as Stack;
}

describe("toMoveBranchWarning", () => {
	test("returns undefined when tearing off leaves all stacks applied", () => {
		const beforeAppliedStackCount = 3;
		const afterAppliedStackCount = 4;
		const unappliedStackCount = Math.max(0, beforeAppliedStackCount + 1 - afterAppliedStackCount);

		expect(toMoveBranchWarning(unappliedStackCount)).toBeUndefined();
	});

	test("renders a singular warning message when one stack is unapplied", () => {
		const beforeAppliedStackCount = 3;
		const afterAppliedStackCount = 3;
		const unappliedStackCount = Math.max(0, beforeAppliedStackCount + 1 - afterAppliedStackCount);
		const warning = toMoveBranchWarning(unappliedStackCount);

		if (!warning || warning.type !== "warning") {
			throw new Error("Expected a warning drop result");
		}

		expect(warning).toMatchObject({
			type: "warning",
			title: "Heads up: We had to unapply some stacks to move this branch",
			testId: TestId.StacksUnappliedToast,
		});
		expect(warning.message).toContain("1 stack");
	});

	test("renders a plural warning message when multiple stacks are unapplied", () => {
		const beforeAppliedStackCount = 3;
		const afterAppliedStackCount = 2;
		const unappliedStackCount = Math.max(0, beforeAppliedStackCount + 1 - afterAppliedStackCount);
		const warning = toMoveBranchWarning(unappliedStackCount);

		if (!warning || warning.type !== "warning") {
			throw new Error("Expected a warning drop result");
		}

		expect(warning.message).toContain("2 stacks");
	});
});

describe("cherryPickTargets", () => {
	test("identifies each stack by its top branch and counts its branches", () => {
		const targets = cherryPickTargets([
			stackWithBranches("feature", "base"),
			stackWithBranches("solo"),
		]);

		expect(targets).toEqual([
			{ branchName: "feature", branchCount: 2 },
			{ branchName: "solo", branchCount: 1 },
		]);
	});

	test("skips stacks whose top branch lost its name", () => {
		const targets = cherryPickTargets([
			stackWithBranches(null, "base"),
			stackWithBranches("feature"),
		]);

		expect(targets).toEqual([{ branchName: "feature", branchCount: 1 }]);
	});
});
