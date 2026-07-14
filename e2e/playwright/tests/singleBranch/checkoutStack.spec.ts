import { expectCurrentBranchChip } from "./helpers.ts";
import { assertBranch } from "../../src/branch.ts";
import { applyUpstream, openWorkspace } from "../../src/setup.ts";
import { test } from "../../src/test.ts";
import { clickByTestId, getByTestId, stack, waitForTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";

test.describe("single-branch mode enabled", () => {
	test.use({
		gitbutlerOptions: {
			config: {
				onboardingComplete: true,
				featureFlags: { singleBranch: true },
			},
		},
	});

	// Checking out a stack branch leaves the managed workspace; the workspace view and the
	// branches page then reflect just that stack. Applying the other branch from the branches
	// page rebuilds a fresh workspace from the current branch plus the applied one.
	test("checking out one of two stacks shows only that stack, and re-applying the other restores both", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-stacks.sh");
		await applyUpstream(gitbutler, "branch1", "branch2");
		await openWorkspace(page);

		const localClone = gitbutler.pathInWorkdir("local-clone");

		// Initial workspace: two independent stacks on gitbutler/workspace.
		await assertBranch("gitbutler/workspace", localClone);
		await expect(stack(page)).toHaveCount(2);

		// External `git checkout` of one stack branch — only that stack stays in view.
		await gitbutler.runScript("checkout-branch.sh", ["branch1", "local-clone"]);
		await assertBranch("branch1", localClone);

		await expect(getByTestId(page, "workspace-view")).toBeVisible();
		await expectCurrentBranchChip(page, "branch1");
		await expect(stack(page)).toHaveCount(1);
		await expect(stack(page, "branch1")).toHaveCount(1);
		await expect(stack(page, "branch2")).toHaveCount(0);

		// Applying the other branch re-enters the managed workspace with both stacks.
		await clickByTestId(page, "navigation-branches-button");
		await getByTestId(page, "branch-list-card").filter({ hasText: "branch2" }).click();
		await clickByTestId(page, "branches-view-apply-branch-button");
		await waitForTestId(page, "workspace-view");

		await assertBranch("gitbutler/workspace", localClone);
		await expectCurrentBranchChip(page, "gitbutler/workspace");
		await expect(stack(page)).toHaveCount(2);
		await expect(stack(page, "branch1")).toHaveCount(1);
		await expect(stack(page, "branch2")).toHaveCount(1);
	});

	// The rebuilt workspace contains strictly the current branch plus the applied one: a third stack
	// that was applied before checking out is dropped (it stays available on the branches page).
	test("rebuilding the workspace drops stacks other than the current and applied branch", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-stacks.sh");
		await applyUpstream(gitbutler, "branch1", "branch2", "branch3");
		await openWorkspace(page);

		const localClone = gitbutler.pathInWorkdir("local-clone");

		await assertBranch("gitbutler/workspace", localClone);
		await expect(stack(page)).toHaveCount(3);

		// Leave the workspace by checking out branch1; only branch1 stays in view.
		await gitbutler.runScript("checkout-branch.sh", ["branch1", "local-clone"]);
		await assertBranch("branch1", localClone);
		await expect(stack(page)).toHaveCount(1);

		// Apply branch2: the workspace rebuilds from branch1 + branch2 only — branch3 is dropped.
		await clickByTestId(page, "navigation-branches-button");
		await getByTestId(page, "branch-list-card").filter({ hasText: "branch2" }).click();
		await clickByTestId(page, "branches-view-apply-branch-button");
		await waitForTestId(page, "workspace-view");

		await assertBranch("gitbutler/workspace", localClone);
		await expect(stack(page)).toHaveCount(2);
		await expect(stack(page, "branch1")).toHaveCount(1);
		await expect(stack(page, "branch2")).toHaveCount(1);
		await expect(stack(page, "branch3")).toHaveCount(0);
	});
});
