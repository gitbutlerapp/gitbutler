import {
	branchHeader,
	expectCurrentBranchChip,
	openSingleBranchWorkspace,
	SINGLE_BRANCH_NAME,
} from "./helpers.ts";
import { assertBranch, assertCommitSubjects, branchTip } from "../../src/branch.ts";
import { expect } from "../../src/expect.ts";
import { openWorkspace } from "../../src/setup.ts";
import { test } from "../../src/test.ts";
import { getByTestId, waitForTestId, waitForTestIdToNotExist } from "../../src/util.ts";

test.describe("single-branch mode disabled", () => {
	test("does not show the current-branch chip after leaving gitbutler/workspace", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await openWorkspace(page);

		const localClone = gitbutler.pathInWorkdir("local-clone");
		await assertBranch("gitbutler/workspace", localClone);

		await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
		await assertBranch("master", localClone);

		await expect(getByTestId(page, "workspace-view")).toBeVisible();
		await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toBeVisible();
		await expect(getByTestId(page, "chrome-header-current-branch")).toHaveCount(0);
	});
});

test.describe("single-branch mode enabled", () => {
	test.use({
		gitbutlerOptions: {
			config: {
				onboardingComplete: true,
				featureFlags: { singleBranch: true },
			},
		},
	});

	test("keeps the workspace UI visible on a normal branch and can switch back", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await openWorkspace(page);

		const localClone = gitbutler.pathInWorkdir("local-clone");
		await assertBranch("gitbutler/workspace", localClone);

		await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
		await assertBranch("master", localClone);

		await expect(getByTestId(page, "workspace-view")).toBeVisible();
		await expectCurrentBranchChip(page, "master");

		const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
		await switchButton.click();

		await assertBranch("gitbutler/workspace", localClone);
		await waitForTestIdToNotExist(page, "chrome-header-switch-back-to-workspace-button");
		await expectCurrentBranchChip(page, "gitbutler/workspace");
	});

	test("can open the app directly on a configured normal branch", async ({ page, gitbutler }) => {
		await gitbutler.runScript("project-in-single-branch-mode.sh");

		const localClone = gitbutler.pathInWorkdir("local-clone");
		await assertBranch(SINGLE_BRANCH_NAME, localClone);

		await openSingleBranchWorkspace(page);

		await assertBranch(SINGLE_BRANCH_NAME, localClone);
		await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
		await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toBeVisible();
		await assertCommitSubjects(
			["single-branch: add file", "single-branch: second commit", "single-branch: first commit"],
			localClone,
		);
	});

	test("shows a new externally checked-out branch at the target commit", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		const localClone = gitbutler.pathInWorkdir("local-clone");
		await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
		await assertBranch("master", localClone);
		expect(branchTip("master", localClone)).toBe(branchTip("origin/master", localClone));

		await openSingleBranchWorkspace(page);
		await expect(branchHeader(page, "master")).toBeVisible();

		const branchName = "external-branch-at-target";
		await gitbutler.runScript("project-with-remote-branches__checkout-new-branch-at-target.sh", [
			"local-clone",
			branchName,
		]);

		await assertBranch(branchName, localClone);
		expect(branchTip(branchName, localClone)).toBe(branchTip("origin/master", localClone));
		await expectCurrentBranchChip(page, branchName);
		await expect(branchHeader(page, branchName)).toBeVisible();
		await expect(branchHeader(page, "master")).toHaveCount(0);
	});
});
