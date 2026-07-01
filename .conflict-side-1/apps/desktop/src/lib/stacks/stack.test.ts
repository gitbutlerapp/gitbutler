import { toMoveBranchWarning } from "$lib/stacks/stack";
import { TestId } from "@gitbutler/ui";
import { describe, expect, test } from "vitest";

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
