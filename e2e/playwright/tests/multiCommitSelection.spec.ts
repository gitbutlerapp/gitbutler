import { applyUpstream, openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	commitRow,
	getByTestId,
	MOD_KEY,
	waitForElementToStabilize,
	waitForTestId,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

/**
 * Apply branch1 (4 commits) and open the workspace.
 */
async function applyBranch1(gitbutler: GitButler, page: Page) {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);
	await expect(commitRow(page)).toHaveCount(4);
}

test("should select multiple commits with Cmd/Ctrl+Click", async ({ page, gitbutler }) => {
	await applyBranch1(gitbutler, page);

	const first = commitRow(page, "branch1: first commit");
	const second = commitRow(page, "branch1: second commit");
	const third = commitRow(page, "branch1: third commit");

	await first.click();
	await expect(getByTestId(page, "commit-drawer")).toBeVisible();

	await third.click({ modifiers: [MOD_KEY] });

	await expect(first).toHaveClass(/\bselected\b/);
	await expect(third).toHaveClass(/\bselected\b/);
	await expect(second).not.toHaveClass(/\bselected\b/);
});

test("should select a range of commits with Shift+Click", async ({ page, gitbutler }) => {
	await applyBranch1(gitbutler, page);

	await commitRow(page, "branch1: fourth commit").click();
	await commitRow(page, "branch1: first commit").click({ modifiers: ["Shift"] });

	const allCommits = commitRow(page);
	const count = await allCommits.count();
	for (let i = 0; i < count; i++) {
		await expect(allCommits.nth(i)).toHaveClass(/\bselected\b/);
	}
});

test("should toggle individual commits with Cmd/Ctrl+Click", async ({ page, gitbutler }) => {
	await applyBranch1(gitbutler, page);

	const first = commitRow(page, "branch1: first commit");
	const second = commitRow(page, "branch1: second commit");

	await first.click();
	await expect(first).toHaveClass(/\bselected\b/);

	await second.click({ modifiers: [MOD_KEY] });
	await expect(first).toHaveClass(/\bselected\b/);
	await expect(second).toHaveClass(/\bselected\b/);

	// Toggle first off
	await first.click({ modifiers: [MOD_KEY] });
	await expect(first).not.toHaveClass(/\bselected\b/);
	await expect(second).toHaveClass(/\bselected\b/);
});

test("should show multi-select context menu with squash and uncommit", async ({
	page,
	gitbutler,
}) => {
	await applyBranch1(gitbutler, page);

	const first = commitRow(page, "branch1: first commit");
	const second = commitRow(page, "branch1: second commit");

	await first.click();
	await second.click({ modifiers: [MOD_KEY] });
	await first.click({ button: "right" });

	const squashItem = await waitForTestId(page, "commit-row-context-menu-squash-selected");
	await waitForElementToStabilize(page, squashItem);
	await expect(squashItem).toContainText("Squash 2 commits");

	const uncommitItem = getByTestId(page, "commit-row-context-menu-uncommit-selected");
	await expect(uncommitItem).toContainText("Uncommit 2 commits");

	// Single-commit items should NOT be visible
	await expect(page.getByTestId("commit-row-context-menu-edit-message-menu-btn")).toHaveCount(0);
	await expect(page.getByTestId("commit-row-context-menu-edit-commit")).toHaveCount(0);
});

test("should collapse changed files when multiple commits are selected", async ({
	page,
	gitbutler,
}) => {
	await applyBranch1(gitbutler, page);

	await commitRow(page, "branch1: first commit").click();
	const changedFilesContainer = page.locator(".changed-files-container");
	await expect(changedFilesContainer).toBeVisible();

	await commitRow(page, "branch1: second commit").click({ modifiers: [MOD_KEY] });
	await expect(changedFilesContainer).not.toBeVisible();
});

test("should squash 3 selected commits via context menu", async ({ page, gitbutler }) => {
	test.setTimeout(120_000);
	await applyBranch1(gitbutler, page);

	const second = commitRow(page, "branch1: second commit");
	await second.click();
	await commitRow(page, "branch1: third commit").click({ modifiers: [MOD_KEY] });
	await commitRow(page, "branch1: fourth commit").click({ modifiers: [MOD_KEY] });

	await second.click({ button: "right" });

	const squashItem = await waitForTestId(page, "commit-row-context-menu-squash-selected");
	await waitForElementToStabilize(page, squashItem);
	await expect(squashItem).toContainText("Squash 3 commits");
	await squashItem.click();

	// Local history changed → upstream divergence action becomes available.
	await expect(getByTestId(page, "upstream-commits-integrate-button")).toBeVisible();
	await expect(commitRow(page, "branch1: first commit")).toBeVisible();
});

test("should uncommit 3 selected commits via context menu", async ({ page, gitbutler }) => {
	test.setTimeout(120_000);
	await applyBranch1(gitbutler, page);

	const fourth = commitRow(page, "branch1: fourth commit");
	await fourth.click();
	await commitRow(page, "branch1: third commit").click({ modifiers: [MOD_KEY] });
	await commitRow(page, "branch1: second commit").click({ modifiers: [MOD_KEY] });

	await fourth.click({ button: "right" });

	const uncommitItem = await waitForTestId(page, "commit-row-context-menu-uncommit-selected");
	await waitForElementToStabilize(page, uncommitItem);
	await expect(uncommitItem).toContainText("Uncommit 3 commits");
	await uncommitItem.click();

	await expect(getByTestId(page, "uncommitted-changes-header")).toBeVisible();
	await expect(commitRow(page, "branch1: first commit")).toBeVisible();
});

test("should deselect all when clicking a commit without modifier", async ({ page, gitbutler }) => {
	await applyBranch1(gitbutler, page);

	const first = commitRow(page, "branch1: first commit");
	const second = commitRow(page, "branch1: second commit");
	const third = commitRow(page, "branch1: third commit");

	await first.click();
	await second.click({ modifiers: [MOD_KEY] });
	await expect(first).toHaveClass(/\bselected\b/);
	await expect(second).toHaveClass(/\bselected\b/);

	await third.click();
	await expect(third).toHaveClass(/\bselected\b/);
	await expect(first).not.toHaveClass(/\bselected\b/);
	await expect(second).not.toHaveClass(/\bselected\b/);
});
