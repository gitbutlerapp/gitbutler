import { expectCurrentBranchChip } from "./helpers.ts";
import {
	assertBranch,
	assertCommitSubjects,
	assertDirtyWorktree,
	assertGitConfigValue,
	assertRefDoesNotExist,
} from "../../src/branch.ts";
import { gotoOnboarding } from "../../src/setup.ts";
import { test } from "../../src/test.ts";
import { clickByTestId, getByTestId, mockPickDirectory, waitForTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

// The single-branch counterpart of the onboarding test in ../addingAProject.spec.ts:
// setting the target must only write project metadata, leaving the user on the branch
// they were on instead of moving them onto gitbutler/workspace.
test("adding a project sets the target without leaving the checked out branch", async ({
	page,
	gitbutler,
}) => {
	const projectPath = gitbutler.pathInWorkdir("local-with-changes/");
	await gitbutler.runScript("project-with-commit-and-uncommitted-changes.sh");
	await gotoOnboarding(page);

	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");
	await waitForTestId(page, "project-setup-page");

	// The setup page must not promise the branch switch the flag turns off.
	await expect(page.getByText(/GitButler switches your active branch/)).toHaveCount(0);

	await clickByTestId(page, "set-base-branch");
	await waitForTestId(page, "workspace-view");

	// The user is still on master with their uncommitted changes; no workspace
	// branch was created or checked out, and no workspace commit was made.
	await assertBranch("master", projectPath);
	await expectCurrentBranchChip(page, "master");
	await assertDirtyWorktree(projectPath);
	await assertRefDoesNotExist("refs/heads/gitbutler/workspace", projectPath);
	await assertCommitSubjects(["Second commit on main branch", "Initial commit"], projectPath);

	// The target itself was configured, just without entering the workspace.
	await assertGitConfigValue(
		"gitbutler.project.targetRef",
		"refs/remotes/origin/master",
		projectPath,
	);

	// Entering the managed workspace stays available as an explicit action.
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toBeVisible();
});
