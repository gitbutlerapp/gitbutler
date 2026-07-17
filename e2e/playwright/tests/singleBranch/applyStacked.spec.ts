import { openSingleBranchWorkspace } from "./helpers.ts";
import { assertSymbolicHead } from "../../src/branch.ts";
import { test } from "../../src/test.ts";
import { clickByTestId, commitRow, getByTestId, waitForTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";
import { execFileSync } from "node:child_process";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("stacks a conflicting branch without entering the managed workspace", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-conflicting-apply.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	const destinationBefore = git(localClone, ["rev-parse", "refs/heads/branch-a"]);
	const remoteIncomingBefore = git(localClone, ["rev-parse", "refs/remotes/origin/branch-b"]);
	await openSingleBranchWorkspace(page);

	await clickByTestId(page, "navigation-branches-button");
	await waitForTestId(page, "branches-view");
	await getByTestId(page, "branch-list-card").filter({ hasText: "branch-b" }).click();
	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "branch-apply-stacking-modal");
	await clickByTestId(page, "branch-apply-stacking-modal-action-button");
	await waitForTestId(page, "workspace-view");

	await assertSymbolicHead("branch-b", localClone);
	expect(git(localClone, ["branch", "--list", "gitbutler/workspace"])).toBe("");
	expect(git(localClone, ["rev-parse", "refs/heads/branch-a"])).toBe(destinationBefore);
	const incomingAfter = git(localClone, ["rev-parse", "refs/heads/branch-b"]);
	expect(incomingAfter).not.toBe(remoteIncomingBefore);
	expect(git(localClone, ["rev-parse", "refs/remotes/origin/branch-b"])).toBe(remoteIncomingBefore);
	expect(git(localClone, ["rev-parse", "refs/heads/branch-b^"])).toBe(destinationBefore);

	const branchCards = getByTestId(page, "branch-card");
	await expect(branchCards).toHaveCount(2);
	await expect(branchCards.nth(0)).toContainText("branch-b");
	await expect(branchCards.nth(1)).toContainText("branch-a");
	await commitRow(page, "branch-b: conflicting change").click();
	await expect(getByTestId(page, "commit-drawer-resolve-conflicts-button")).toBeVisible();
});

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}
