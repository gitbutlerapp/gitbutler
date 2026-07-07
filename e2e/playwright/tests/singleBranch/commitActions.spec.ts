import {
	applyBranchFromBranchesView,
	branchHeader,
	createDependentBranch,
	expectCurrentBranchChip,
	openSingleBranchWorkspace,
	setupSingleBranchProject,
	SINGLE_BRANCH_NAME,
} from "./helpers.ts";
import {
	assertBranch,
	assertCleanWorktree,
	assertCommitSubjects,
	assertDirtyWorktree,
	assertSymbolicHead,
} from "../../src/branch.ts";
import {
	openCommitDrawer,
	startEditingCommitMessage,
	updateCommitMessage,
	verifyCommitDrawerContent,
	verifyCommitMessageEditor,
	verifyCommitPlaceholderPosition,
} from "../../src/commit.ts";
import { assertFileContent, unstageAllFiles, writeToFile } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import {
	clickByTestId,
	commitRow,
	dragAndDropByLocator,
	getByTestId,
	stack,
	waitForTestId,
} from "../../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("can commit new changes on the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);

	const fileName = "single_branch_new_file.txt";
	const fileContent = "new single branch content\n";
	writeToFile(gitbutler.pathInWorkdir("local-clone", fileName), fileContent);

	await expect(getByTestId(page, "file-list-item").filter({ hasText: fileName })).toBeVisible();
	await clickByTestId(page, "start-commit-button");
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);
	await getByTestId(page, "file-list-item")
		.filter({ hasText: fileName })
		.locator('input[type="checkbox"]')
		.click();

	const title = "single-branch: commit from e2e";
	const body = "Committed while HEAD is on a normal Git branch.";
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, title, body);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, title)).toBeVisible();
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertFileContent(gitbutler.pathInWorkdir("local-clone", fileName), fileContent);
	await assertCommitSubjects(
		[
			title,
			"single-branch: add file",
			"single-branch: second commit",
			"single-branch: first commit",
		],
		localClone,
	);
});

test("can create an empty dependent branch above the checked-out branch and commit to it", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	const topBranch = "single-branch-dependent-top";
	const bottomTip = git(localClone, ["rev-parse", SINGLE_BRANCH_NAME]);

	await createDependentBranch(page, topBranch);

	await assertBranch(topBranch, localClone);
	await expectCurrentBranchChip(page, topBranch);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(getByTestId(page, "branch-card").filter({ hasText: topBranch })).toBeVisible();
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: SINGLE_BRANCH_NAME }),
	).toBeVisible();
	expect(git(localClone, ["rev-parse", topBranch])).toBe(bottomTip);

	const fileName = "dependent_top_file.txt";
	const fileContent = "new top branch content\n";
	writeToFile(gitbutler.pathInWorkdir("local-clone", fileName), fileContent);

	await expect(getByTestId(page, "file-list-item").filter({ hasText: fileName })).toBeVisible();
	await clickByTestId(page, "start-commit-button");

	const title = "dependent-top: commit from e2e";
	await updateCommitMessage(page, title, "");
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, title)).toBeVisible();
	await assertBranch(topBranch, localClone);
	await assertCleanWorktree(localClone);
	expect(git(localClone, ["rev-parse", `${topBranch}^`])).toBe(bottomTip);
	expect(git(localClone, ["rev-parse", SINGLE_BRANCH_NAME])).toBe(bottomTip);
	await assertCommitSubjects(
		[
			title,
			"single-branch: add file",
			"single-branch: second commit",
			"single-branch: first commit",
		],
		localClone,
	);
});

test("can create an empty branch below another empty branch between empty branches", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	await createDependentBranch(page, "empty-top");
	const emptyMiddle = await createGeneratedBranch(page, localClone, "empty-top", "below");
	const emptyBottom = await createGeneratedBranch(page, localClone, emptyMiddle, "below");
	const insertedBelowMiddle = await createGeneratedBranch(page, localClone, emptyMiddle, "below");

	await expectBranchHeaderOrder(page, [
		"empty-top",
		emptyMiddle,
		insertedBelowMiddle,
		emptyBottom,
		SINGLE_BRANCH_NAME,
	]);
	await assertBranch("empty-top", localClone);
	for (const branchName of ["empty-top", emptyMiddle, insertedBelowMiddle, emptyBottom]) {
		expect(git(localClone, ["rev-parse", branchName])).toBe(
			git(localClone, ["rev-parse", SINGLE_BRANCH_NAME]),
		);
	}
});

test("can create a dependent branch above the checked-out branch via the context menu", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	const baseTip = git(localClone, ["rev-parse", SINGLE_BRANCH_NAME]);

	const above = await createGeneratedBranch(page, localClone, SINGLE_BRANCH_NAME, "above");

	// Creating above the checked-out branch checks the new empty tip out.
	await expectBranchHeaderOrder(page, [above, SINGLE_BRANCH_NAME]);
	await expectCurrentBranchChip(page, above);
	await assertBranch(above, localClone);
	expect(git(localClone, ["rev-parse", above])).toBe(baseTip);
});

test("can create a dependent branch below the checked-out branch via the context menu", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	const baseTip = git(localClone, ["rev-parse", SINGLE_BRANCH_NAME]);

	const below = await createGeneratedBranch(page, localClone, SINGLE_BRANCH_NAME, "below");

	// Creating below the checked-out branch leaves HEAD where it is.
	await expectBranchHeaderOrder(page, [SINGLE_BRANCH_NAME, below]);
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	expect(git(localClone, ["rev-parse", below])).toBe(baseTip);
});

test("persists the ad-hoc branch order across a workspace reload", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	// Build an ordered stack of empty branches; each is created above the current tip.
	await createDependentBranch(page, "empty-lower");
	await createDependentBranch(page, "empty-top");
	const expectedOrder = ["empty-top", "empty-lower", SINGLE_BRANCH_NAME];

	await expectBranchHeaderOrder(page, expectedOrder);
	await expectCurrentBranchChip(page, "empty-top");

	// Reload the app from scratch: the workspace must be rebuilt from the persisted branch-order
	// metadata on disk, not from any in-memory state.
	await openSingleBranchWorkspace(page);

	await expectBranchHeaderOrder(page, expectedOrder);
	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectCurrentBranchChip(page, "empty-top");
	for (const branchName of expectedOrder) {
		expect(localBranches(localClone)).toContain(branchName);
	}
});

test("surfaces an error and creates nothing when a dependent branch name collides", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	const branchesBefore = localBranches(localClone);

	// `single-branch-fixture` exists as a branch, so `single-branch-fixture/child` collides with it
	// and cannot be created. The name is valid client-side, so it reaches the backend and fails.
	await clickByTestId(page, "branch-header-add-dependent-branch-button");
	const modal = await waitForTestId(page, "branch-header-add-dependent-branch-modal");
	await modal.locator("input").fill(`${SINGLE_BRANCH_NAME}/child`);
	await clickByTestId(page, "branch-header-add-dependent-branch-modal-action-button");

	// The failure is surfaced as an error toast.
	await expect(
		page.getByTestId("toast-info-message").filter({ hasText: /error|collides|cannot/i }),
	).toBeVisible();

	// And nothing is left behind: no new branch on disk, and the stack is unchanged.
	await expect(getByTestId(page, "branch-card")).toHaveCount(1);
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
	expect(localBranches(localClone)).toEqual(branchesBefore);
});

test("can remove the checked-out empty branch from the top of a two-branch stack", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	// `empty-top` is created above the base and becomes the checked-out tip.
	await createDependentBranch(page, "empty-top");
	const emptyBottom = await createGeneratedBranch(page, localClone, "empty-top", "below");
	await expectCurrentBranchChip(page, "empty-top");

	await deleteBranchFromHeader(page, "empty-top");

	// Removing the checked-out empty tip lands HEAD on the branch directly below it.
	await expect(branchHeader(page, "empty-top")).toBeHidden();
	await expectCurrentBranchChip(page, emptyBottom);
	await assertBranch(emptyBottom, localClone);
	await expectBranchHeaderOrder(page, [emptyBottom, SINGLE_BRANCH_NAME]);
	expect(localBranches(localClone)).not.toContain("empty-top");
});

test("can remove an empty branch from the middle of a three-branch stack", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	await createDependentBranch(page, "empty-top");
	const emptyMiddle = await createGeneratedBranch(page, localClone, "empty-top", "below");
	const emptyBottom = await createGeneratedBranch(page, localClone, emptyMiddle, "below");
	await expectCurrentBranchChip(page, "empty-top");

	await deleteBranchFromHeader(page, emptyMiddle);

	// Removing a branch that isn't checked out relinks the order and leaves HEAD on the tip.
	await expect(branchHeader(page, emptyMiddle)).toBeHidden();
	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
	await expectBranchHeaderOrder(page, ["empty-top", emptyBottom, SINGLE_BRANCH_NAME]);
	expect(localBranches(localClone)).not.toContain(emptyMiddle);
});

test("can remove an empty branch from the bottom of a two-branch stack", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	await createDependentBranch(page, "empty-top");
	const emptyBottom = await createGeneratedBranch(page, localClone, "empty-top", "below");
	await expectCurrentBranchChip(page, "empty-top");

	await deleteBranchFromHeader(page, emptyBottom);

	await expect(branchHeader(page, emptyBottom)).toBeHidden();
	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
	await expectBranchHeaderOrder(page, ["empty-top", SINGLE_BRANCH_NAME]);
	expect(localBranches(localClone)).not.toContain(emptyBottom);
});

test("can rename the branch we've directly checked out", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	// HEAD is directly on `single-branch-fixture`, which owns commits.
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);

	await renameBranchFromHeader(page, SINGLE_BRANCH_NAME, "renamed-checked-out");

	// Renaming the checked-out branch carries HEAD over to the new name.
	await expect(branchHeader(page, SINGLE_BRANCH_NAME)).toBeHidden();
	await expectCurrentBranchChip(page, "renamed-checked-out");
	await assertBranch("renamed-checked-out", localClone);
	expect(localBranches(localClone)).toContain("renamed-checked-out");
	expect(localBranches(localClone)).not.toContain(SINGLE_BRANCH_NAME);
});

test("can rename branches stacked under the directly checked-out branch", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	// Stack empty branches above the base: [empty-top, empty-lower, single-branch-fixture],
	// with HEAD directly on `empty-top`.
	await createDependentBranch(page, "empty-lower");
	await createDependentBranch(page, "empty-top");
	await expectCurrentBranchChip(page, "empty-top");
	await expectBranchHeaderOrder(page, ["empty-top", "empty-lower", SINGLE_BRANCH_NAME]);

	// Rename a middle branch below the checked-out tip.
	await renameBranchFromHeader(page, "empty-lower", "empty-lower-renamed");
	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
	await expectBranchHeaderOrder(page, ["empty-top", "empty-lower-renamed", SINGLE_BRANCH_NAME]);

	// Rename the base branch below the checked-out tip.
	await renameBranchFromHeader(page, SINGLE_BRANCH_NAME, "base-renamed");
	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
	await expectBranchHeaderOrder(page, ["empty-top", "empty-lower-renamed", "base-renamed"]);

	// The renames landed on disk and HEAD stayed on the tip throughout.
	const branches = localBranches(localClone);
	expect(branches).toEqual(
		expect.arrayContaining(["empty-top", "empty-lower-renamed", "base-renamed"]),
	);
	expect(branches).not.toContain("empty-lower");
	expect(branches).not.toContain(SINGLE_BRANCH_NAME);
});

test("can rename the directly checked-out branch inline", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);

	await renameBranchInline(page, SINGLE_BRANCH_NAME, "inline-renamed");

	// Editing the name in place on the checked-out branch carries HEAD to the new name.
	await expect(branchHeader(page, SINGLE_BRANCH_NAME)).toBeHidden();
	await expectCurrentBranchChip(page, "inline-renamed");
	await assertBranch("inline-renamed", localClone);
	expect(localBranches(localClone)).toContain("inline-renamed");
	expect(localBranches(localClone)).not.toContain(SINGLE_BRANCH_NAME);
});

test("can rename a branch stacked under the checked-out branch inline", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	// [empty-top, single-branch-fixture] with HEAD directly on `empty-top`.
	await createDependentBranch(page, "empty-top");
	await expectCurrentBranchChip(page, "empty-top");

	// Rename the base branch (under the checked-out tip) in place.
	await renameBranchInline(page, SINGLE_BRANCH_NAME, "base-inline-renamed");

	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
	await expectBranchHeaderOrder(page, ["empty-top", "base-inline-renamed"]);
	expect(localBranches(localClone)).toContain("base-inline-renamed");
	expect(localBranches(localClone)).not.toContain(SINGLE_BRANCH_NAME);
});

test("can edit a commit message on the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const originalTitle = "single-branch: add file";
	const newTitle = "single-branch: reworded add file";
	const newBody = "Reworded in single-branch mode.";

	const drawer = await openCommitDrawer(page, originalTitle);
	await startEditingCommitMessage(page, drawer);
	await verifyCommitMessageEditor(page, originalTitle, "");

	await updateCommitMessage(page, newTitle, newBody);
	await clickByTestId(page, "commit-drawer-action-button");

	await verifyCommitDrawerContent(drawer, newTitle, newBody);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertCommitSubjects(
		[newTitle, "single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}

function localBranches(pathToRepo: string): string[] {
	return git(pathToRepo, ["for-each-ref", "--format=%(refname:short)", "refs/heads"])
		.split("\n")
		.filter(Boolean);
}

async function createGeneratedBranch(
	page: Page,
	localClone: string,
	anchorBranchName: string,
	side: "above" | "below",
): Promise<string> {
	const before = new Set(localBranches(localClone));
	await branchHeader(page, anchorBranchName).click({ button: "right" });
	await waitForTestId(page, "branch-header-context-menu");
	await page.getByRole("menuitem", { name: "Create branch" }).click();
	await page.getByRole("button", { name: `Create branch ${side}` }).click();

	await expect
		.poll(
			() => {
				return localBranches(localClone).filter((name) => !before.has(name)).length;
			},
			{
				message: `Expected one generated branch to be created ${side} ${anchorBranchName}`,
				intervals: [100, 200, 500, 1000],
			},
		)
		.toBe(1);
	const branchName = localBranches(localClone).find((name) => !before.has(name))!;
	await expect(branchHeader(page, branchName)).toBeVisible();
	return branchName;
}

async function deleteBranchFromHeader(page: Page, branchName: string): Promise<void> {
	await branchHeader(page, branchName).click({ button: "right" });
	await clickByTestId(page, "branch-header-context-menu-delete");
	const modal = await waitForTestId(page, "branch-header-delete-modal");
	await clickByTestId(page, "branch-header-delete-modal-action-button");
	await expect(modal).toBeHidden();
}

async function renameBranchFromHeader(page: Page, oldName: string, newName: string): Promise<void> {
	await branchHeader(page, oldName).click({ button: "right" });
	await clickByTestId(page, "branch-header-context-menu-rename");
	const modal = await waitForTestId(page, "branch-header-rename-modal");
	await modal.locator("input").fill(newName);
	await clickByTestId(page, "branch-header-rename-modal-action-button");
	await expect(modal).toBeHidden();
	await expect(branchHeader(page, newName)).toBeVisible();
}

async function renameBranchInline(page: Page, oldName: string, newName: string): Promise<void> {
	const input = branchHeader(page, oldName).locator("input.branch-name-input");
	await input.click();
	await input.fill(newName);
	// Enter blurs the field, which commits the rename.
	await input.press("Enter");
	await expect(branchHeader(page, newName)).toBeVisible();
}

async function expectBranchHeaderOrder(page: Page, expectedBranchNames: string[]): Promise<void> {
	await expect
		.poll(
			async () =>
				await page
					.locator('[data-testid="branch-header"]')
					.evaluateAll((headers) =>
						headers.map((header) => header.getAttribute("data-testid-branch-header")),
					),
			{
				message: `Expected branch header order ${JSON.stringify(expectedBranchNames)}`,
				intervals: [100, 200, 500, 1000],
			},
		)
		.toEqual(expectedBranchNames);
}

test("can amend file changes into an existing commit", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const filePath = gitbutler.pathInWorkdir("local-clone", "single_branch_file.txt");
	const amendedContent = "single branch file\namended while in single-branch mode\n";
	writeToFile(filePath, amendedContent);
	await assertDirtyWorktree(localClone);

	const fileLocator = getByTestId(page, "file-list-item").filter({
		hasText: "single_branch_file.txt",
	});
	await expect(fileLocator).toBeVisible();

	await dragAndDropByLocator(page, fileLocator, commitRow(page, "single-branch: add file"));

	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertFileContent(filePath, amendedContent);
	await assertCommitSubjects(
		["single-branch: add file", "single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});

test("can apply another branch after leaving a managed workspace", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-in-single-branch-apply-transition.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);

	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await applyBranchFromBranchesView(page, "branch-to-apply");

	await assertBranch("gitbutler/workspace", localClone);
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
		"gitbutler/workspace",
	);
	await expect(getByTestId(page, "chrome-header-current-branch")).not.toContainText("read-only");
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toHaveCount(0);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: SINGLE_BRANCH_NAME }),
	).toBeVisible();
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: "branch-to-apply" }),
	).toBeVisible();
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: "stale-workspace-branch" }),
	).toHaveCount(0);
	await assertCleanWorktree(localClone);
});

test("rebuilds an enclosed ad-hoc workspace around the current and applied branches", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-enclosed-apply.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);

	await assertBranch("B", localClone);
	await assertSymbolicHead("B", localClone);
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText("B");
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText("read-only");
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toBeVisible();

	await applyBranchFromBranchesView(page, "C");

	await assertBranch("gitbutler/workspace", localClone);
	await assertSymbolicHead("gitbutler/workspace", localClone);
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
		"gitbutler/workspace",
	);
	await expect(getByTestId(page, "chrome-header-current-branch")).not.toContainText("read-only");
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toHaveCount(0);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(stack(page, "A")).toHaveCount(0);
	await expect(stack(page, "B")).toBeVisible();
	await expect(stack(page, "C")).toBeVisible();
	await assertCleanWorktree(localClone);
});

test("re-enters the managed workspace when applying an already-enclosed branch", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-existing-workspace-apply.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch("bug-fix", localClone);
	await assertSymbolicHead("bug-fix", localClone);
	const bugFixTipBeforeApply = git(localClone, ["rev-parse", "bug-fix"]);

	await openSingleBranchWorkspace(page);

	await applyBranchFromBranchesView(page, "feature-foo");

	await assertBranch("gitbutler/workspace", localClone);
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
		"gitbutler/workspace",
	);
	await expect(getByTestId(page, "chrome-header-current-branch")).not.toContainText("read-only");
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toHaveCount(0);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(stack(page, "feature-foo")).toBeVisible();
	await expect(stack(page, "bug-fix")).toBeVisible();
	expect(git(localClone, ["rev-parse", "bug-fix"])).toBe(bugFixTipBeforeApply);
	await assertCleanWorktree(localClone);
});

test("rebuilds managed workspace around checked-out and applied branches", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-existing-workspace-reroot.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch("A", localClone);
	await assertSymbolicHead("A", localClone);
	const aTipBeforeApply = git(localClone, ["rev-parse", "A"]);

	await openSingleBranchWorkspace(page);

	await applyBranchFromBranchesView(page, "C");

	await assertBranch("gitbutler/workspace", localClone);
	await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
		"gitbutler/workspace",
	);
	await expect(getByTestId(page, "chrome-header-current-branch")).not.toContainText("read-only");
	await expect(getByTestId(page, "chrome-header-switch-back-to-workspace-button")).toHaveCount(0);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(stack(page, "A")).toBeVisible();
	await expect(stack(page, "B")).toHaveCount(0);
	await expect(stack(page, "C")).toBeVisible();
	expect(git(localClone, ["rev-parse", "A"])).toBe(aTipBeforeApply);
	await assertCleanWorktree(localClone);
});
