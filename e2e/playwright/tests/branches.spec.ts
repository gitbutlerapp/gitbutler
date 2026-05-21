import { createNewBranch, deleteBranch, unapplyStack } from "../src/branch.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	fillByTestId,
	getByTestId,
	stack,
	textEditorFillByTestId,
	waitForTestId,
	waitForTestIdToNotExist,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { existsSync, readFileSync, writeFileSync } from "fs";

/**
 * Navigate to the branches page and assert the standard 3-card layout from
 * `project-with-remote-branches.sh`. Filters on the `origin/master` header
 * because workspace stack headers also use the `branch-header` test id.
 */
async function gotoBranchesView(page: Page) {
	await clickByTestId(page, "navigation-branches-button");
	const originHeader = getByTestId(page, "branch-header").filter({ hasText: "origin/master" });
	await expect(originHeader).toBeVisible();

	const cards = getByTestId(page, "branch-list-card");
	await expect(cards).toHaveCount(3);
}

/**
 * Navigate to the branches page and apply a branch from its card.
 */
async function applyBranchFromBranchesView(page: Page, branchName: string) {
	await gotoBranchesView(page);

	await getByTestId(page, "branch-list-card").filter({ hasText: branchName }).click();
	await waitForTestId(page, "branches-view-delete-local-branch-button");

	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");
}

async function syncAndIntegrate(page: Page) {
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "upstream-commits-integrate-button");
	await waitForTestId(page, "branch-integration-apply-button");
	await clickByTestId(page, "branch-integration-apply-button");
	await waitForTestIdToNotExist(page, "branch-integration-modal");
}

test("should be able to apply a remote branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await applyBranchFromBranchesView(page, "branch1");

	await expect(stack(page, "branch1")).toHaveCount(1);
	await expect(commitRow(page)).toHaveCount(2);
});

test("should be able to apply a remote branch and integrate the remote changes - simple", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await applyBranchFromBranchesView(page, "branch1");
	await expect(commitRow(page)).toHaveCount(2);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await syncAndIntegrate(page);

	await expect(commitRow(page)).toHaveCount(3);
});

test("should be able to apply a remote branch and integrate the remote changes - create commit", async ({
	page,
	gitbutler,
}) => {
	const fileCPath = gitbutler.pathInWorkdir("local-clone/c_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page, "branch1")).toHaveCount(1);
	await expect(commitRow(page)).toHaveCount(2);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await clickByTestId(page, "sync-button");

	// New local commit
	writeFileSync(fileCPath, "This is file C\n", { flag: "w" });
	await clickByTestId(page, "start-commit-button");
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "New local commit: adding file C";
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(page, "commit-drawer-description-input", "CCCCCCC");
	await clickByTestId(page, "commit-drawer-action-button");

	// Integrate upstream commits on top
	await clickByTestId(page, "upstream-commits-integrate-button");
	await waitForTestId(page, "branch-integration-apply-button");
	await clickByTestId(page, "branch-integration-apply-button");
	await waitForTestIdToNotExist(page, "branch-integration-modal");

	const commits = commitRow(page);
	await expect(commits).toHaveCount(4);
	await expect(commits.nth(0)).toContainText(newCommitMessage);
});

test("should be able to apply a remote branch and integrate the remote changes - conflict", async ({
	page,
	gitbutler,
}) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(2);

	writeFileSync(filePath, "conflicting change\n", { flag: "a" });

	await clickByTestId(page, "start-commit-button");
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "Conflicting change commit";
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(
		page,
		"commit-drawer-description-input",
		"This should be oh-so-bad 🤭",
	);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page)).toHaveCount(3);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await syncAndIntegrate(page);

	const commits = commitRow(page);
	await expect(commits).toHaveCount(4);

	const conflictedCommit = commitRow(page, newCommitMessage);
	await conflictedCommit.click();
	await clickByTestId(page, "commit-drawer-resolve-conflicts-button");
	await waitForTestId(page, "edit-mode");

	expect(readFileSync(filePath, "utf-8")).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
<<<<<` +
			`<< New base: branch1: third commit
branch1 commit 3
||||||| Common ancestor
=======
conflicting change
>>>>>>> Current commit: Conflicting change commit
`,
	);

	const resolved = `foo
bar
baz
branch1 commit 1
branch1 commit 2
branch1 commit 3
conflicting change
`;
	writeFileSync(filePath, resolved, { flag: "w" });

	await clickByTestId(page, "edit-mode-save-and-exit-button");
	await waitForTestId(page, "workspace-view");

	await expect(commitRow(page)).toHaveCount(4);
	expect(readFileSync(filePath, "utf-8")).toEqual(resolved);
});

test("should be able gracefully handle adding a branch that is ahead of our target commit", async ({
	page,
	gitbutler,
}) => {
	const fileBPath = gitbutler.pathInWorkdir("local-clone/b_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base-and-branch.sh");
	await clickByTestId(page, "sync-button");

	await applyBranchFromBranchesView(page, "branch1");

	// 3 commits from branch1 + 1 base commit
	await expect(commitRow(page)).toHaveCount(4);
	expect(existsSync(fileBPath)).toBe(true);
});

// TODO: The integrate-upstream-commits-button assertion fails because target.sha
// is now set correctly after upstream integration, so no further integration is detected.
test.skip("should be able gracefully handle adding a branch that is behind of our target commit", async ({
	page,
	gitbutler,
}) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
	await waitForTestIdToNotExist(page, "integrate-upstream-commits-button");

	await applyBranchFromBranchesView(page, "branch1");
	await expect(getByTestId(page, "integrate-upstream-commits-button")).toBeVisible();

	await expect(commitRow(page)).toHaveCount(2);
	const conflictedCommit = commitRow(page, "branch1: first commit");
	await conflictedCommit.click();
	await expect(getByTestId(page, "commit-drawer-resolve-conflicts-button")).toHaveCount(0);
	expect(readFileSync(filePath, "utf-8")).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
`,
	);
});

test("should handle gracefully applying two conflicting branches", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(2);

	await gotoBranchesView(page);
	await getByTestId(page, "branch-list-card").filter({ hasText: "branch2" }).click();
	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");

	await waitForTestId(page, "stacks-unapplied-toast");
});

test("should update the stale selection of an unexisting branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await gotoBranchesView(page);
	await getByTestId(page, "branch-list-card").filter({ hasText: "branch1" }).click();

	await clickByTestId(page, "navigation-workspace-button");
	await waitForTestId(page, "workspace-view");
	await expect(stack(page)).toHaveCount(1);

	// branch1 was merged in the forge — sync and integrate it away.
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
	await waitForTestIdToNotExist(page, "integrate-upstream-action-button");

	await waitForTestIdToNotExist(page, "stack");

	await gotoBranchesView(page);

	// We don't prune, so 3 branches remain, but branch1 is not selected anymore.
	const cardsAfter = getByTestId(page, "branch-list-card");
	await expect(cardsAfter.filter({ hasText: "branch1" })).not.toHaveClass(/\bselected\b/);
	await expect(getByTestId(page, "current-origin-list-card")).toHaveClass(/\bselected\b/);
});

test("should be able to delete a local branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1", "branch3");
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header")).toHaveCount(2);

	await deleteBranch(page, "branch1");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(1);
	const headers = getByTestId(page, "branch-header");
	await expect(headers).toHaveCount(1);
	await expect(headers.first()).toContainText("branch3");
});

test("should be able to delete an empty local branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await createNewBranch(page, "new-branch");
	await expect(stack(page)).toHaveCount(1);

	await deleteBranch(page, "new-branch");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(0);
	await expect(getByTestId(page, "branch-header")).toHaveCount(0);
});

test("should be able to unapply a stack", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header").filter({ hasText: "branch1" })).toBeVisible();

	await unapplyStack(page, "branch1");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(0);
	await expect(getByTestId(page, "branch-header").filter({ hasText: "branch1" })).toHaveCount(0);
});

test("should be able to move a branch when origin/master has advanced past the fork point", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1", "branch3");

	// Advance origin/master past the fork point of branch1/branch3 so the old
	// fork point becomes an unnamed segment in the graph.
	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await gitbutler.runScript("fetch-in-clone.sh", ["local-clone"]);
	// Move branch3 on top of branch1 — must succeed even with a nameless base segment.
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);

	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header")).toHaveCount(2);
});
