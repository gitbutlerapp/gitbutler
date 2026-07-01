import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, commitRow, dragAndDropByLocator, getByTestId } from "../src/util.ts";
import { expect } from "@playwright/test";
import { writeFileSync } from "fs";

test("should show commit-failed modal when amending causes a conflict", async ({
	page,
	gitbutler,
}) => {
	// Set up a project with a branch that has two commits modifying the same
	// line of a 20-line file. Amending the first commit with a conflicting
	// worktree change will cause a cherry-pick merge conflict when rebasing
	// the second commit on top.
	await gitbutler.runScript("project-with-conflicting-commits.sh");
	await applyUpstream(gitbutler, "conflicting-branch");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(2);

	// HEAD has "JULIET-SECOND" on line 10 (from commit 2).
	// Changing it to "JULIET-WORKTREE" will, when amended into commit 1, make
	// the rebase of commit 2 fail because it expects "JULIET-FIRST".
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");
	const newContent =
		[
			"alpha",
			"bravo",
			"charlie",
			"delta",
			"echo",
			"foxtrot",
			"golf",
			"hotel",
			"india",
			"JULIET-WORKTREE",
			"kilo",
			"lima",
			"mike",
			"november",
			"oscar",
			"papa",
			"quebec",
			"romeo",
			"sierra",
			"tango",
		].join("\n") + "\n";
	writeFileSync(filePath, newContent);

	const fileLocator = page
		.getByTestId("uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: "a_file" });
	await expect(fileLocator).toBeVisible();

	const firstCommit = commitRow(page, "Change juliet to JULIET-FIRST");
	await dragAndDropByLocator(page, fileLocator, firstCommit);

	const modal = getByTestId(page, "global-modal-commit-failed");
	await expect(modal).toBeVisible();
	await expect(modal).toContainText(/changes were not committed|Failed to create commit/);

	await clickByTestId(page, "global-modal-action-button");
	await expect(modal).not.toBeVisible();
});

test("should squash commits via drag-and-drop without errors", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(4);

	const topCommit = commitRow(page, "branch1: fourth commit");
	const secondCommit = commitRow(page, "branch1: third commit");
	await dragAndDropByLocator(page, topCommit, secondCommit);

	// After squashing, the branch shows upstream divergence — the squash worked.
	await expect(getByTestId(page, "upstream-commits-commit-action")).toBeVisible();
	await expect(commitRow(page, "branch1: first commit")).toBeVisible();
	await expect(getByTestId(page, "global-modal-commit-failed")).not.toBeVisible();
});
