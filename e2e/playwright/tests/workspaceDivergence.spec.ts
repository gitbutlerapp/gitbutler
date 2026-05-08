import { assertBranch, createNewBranch } from "../src/branch.ts";
import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, getByTestId, waitForTestId } from "../src/util.ts";
import { expect } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("should detect diverged stack ref and allow resolution via modal", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Set up a project with remote branches and apply one to the workspace.
	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	// Workspace should load with one stack.
	await waitForTestId(page, "workspace-view");
	const stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);

	// Externally move the stack ref to the base commit (simulates corruption).
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);

	// Reload the page to trigger fresh divergence detection.
	await page.reload();
	await waitForTestId(page, "workspace-view");

	// The divergence modal should appear (may take a moment for the query to complete).
	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	await expect(modal).toBeVisible();

	// There should be one divergence item.
	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(1);

	// Click the Apply button to resolve.
	await clickByTestId(page, "workspace-divergence-modal-action-button");

	// Modal should close.
	await expect(modal).not.toBeVisible();
});

test("should detect multiple diverged refs when several stacks are corrupted", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Set up with independent branches and apply two of them.
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");

	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	// Move both refs below base (simulates external corruption of multiple stacks).
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch2", "local-clone"]);

	// Reload to trigger fresh divergence detection.
	await page.reload();
	await waitForTestId(page, "workspace-view");

	// The modal should show both diverged stacks.
	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	await expect(modal).toBeVisible();

	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(2);

	// Resolve both (default is Exclude for below-base).
	await clickByTestId(page, "workspace-divergence-modal-action-button");

	// Modal should close and both stacks should be excluded.
	await expect(modal).not.toBeVisible();
	await expect(getByTestId(page, "stack")).toHaveCount(0);

	// Workspace should still be functional — create a new branch.
	await createNewBranch(page, "fresh-branch");
	await expect(getByTestId(page, "stack")).toHaveCount(1);
});

test("should allow creating a new branch after resolving divergence", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(1);

	// Corrupt the ref.
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);

	// Reload to trigger divergence detection.
	await page.reload();
	await waitForTestId(page, "workspace-view");

	// Wait for the divergence modal and resolve with default (Exclude for below-base).
	await waitForTestId(page, "workspace-divergence-modal");
	await clickByTestId(page, "workspace-divergence-modal-action-button");

	// After excluding the diverged stack, workspace should have no stacks.
	await expect(getByTestId(page, "stack")).toHaveCount(0);

	// The key assertion: creating a new branch should succeed
	// (this is the operation that previously failed with "target commit already belongs to another branch").
	await createNewBranch(page, "fresh-branch");
	await expect(getByTestId(page, "stack")).toHaveCount(1);
});

test("should show divergence modal with multiple refs when switching back to workspace", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Set up workspace with two independent stacks.
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	// Leave workspace.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await assertBranch("master", workdir + "/local-clone");

	// Move both refs below base while outside workspace (simulates CI force-push).
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch2", "local-clone"]);

	// Click "Back to workspace".
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	// Modal should appear with both diverged stacks.
	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(2);

	// Resolve both and verify workspace loads with no stacks.
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");
	await assertBranch("gitbutler/workspace", workdir + "/local-clone");
	await expect(getByTestId(page, "stack")).toHaveCount(0);
});

test("should only show divergence for the affected stack when one of two is moved", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Set up workspace with two independent stacks.
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	// Leave workspace.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await assertBranch("master", workdir + "/local-clone");

	// Only move branch1 — branch2 stays where it was.
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);

	// Click "Back to workspace".
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	// Modal should appear with only one diverged stack.
	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(1);

	// Resolve and verify workspace loads. branch2 should survive (it wasn't diverged).
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");
	await assertBranch("gitbutler/workspace", workdir + "/local-clone");
	await expect(getByTestId(page, "stack")).toHaveCount(1);
});

test("should show divergence modal when switching back to workspace with diverged refs", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Set up a project with a remote branch applied to the workspace.
	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(1);

	// Switch to master branch (leaving workspace).
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await assertBranch("master", workdir + "/local-clone");

	// Move the stack ref below base while outside workspace.
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch1", "local-clone"]);

	// The "Back to workspace" button should appear.
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	// The divergence modal should appear instead of silently switching back.
	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	await expect(modal).toBeVisible();

	// There should be one divergence item for branch1.
	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(1);

	// Resolve (default for below-base is Exclude) and verify we're back in workspace.
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();

	// Wait for workspace to fully reload after resolution.
	await waitForTestId(page, "workspace-view");

	// Should be back on workspace with no stacks (the diverged one was excluded).
	await assertBranch("gitbutler/workspace", workdir + "/local-clone");
	await expect(getByTestId(page, "stack")).toHaveCount(0);
});

test("excluding a diverged stack removes its files from the working directory", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// project-with-stacks: branch2 creates b_file (independent from branch1's a_file).
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(1);

	const repoDir = path.join(workdir, "local-clone");

	// b_file should exist in the working directory (branch2's content).
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(true);

	// Leave workspace and move the ref below base.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch2", "local-clone"]);

	// Switch back — modal should appear.
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });
	await expect(modal.getByTestId("workspace-divergence-modal-divergence-item")).toHaveCount(1);

	// Resolve with Exclude (default for below-base).
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");

	// b_file should no longer exist — the excluded branch's changes are gone.
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(false);
	// a_file should still exist (base content).
	expect(fs.existsSync(path.join(repoDir, "a_file"))).toBe(true);
});

test("excluding one diverged stack preserves the healthy stack's files", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// branch2 creates b_file, branch3 creates c_file — independent files.
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch3", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	const repoDir = path.join(workdir, "local-clone");

	// Both stacks' files should be present.
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(true);
	expect(fs.existsSync(path.join(repoDir, "c_file"))).toBe(true);

	// Leave workspace and move branch3's ref below base.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await gitbutler.runScript("move-stack-ref-below-base.sh", ["branch3", "local-clone"]);

	// Switch back — modal shows diverged stacks. branch3 was moved below base (Exclude),
	// branch2 may also appear as diverged if `but apply` rebased its commits (Include as-is).
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });

	// Accept defaults: branch3 → Exclude, branch2 → Include as-is (if present).
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");

	// branch2 was included — b_file should still exist with its content.
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(true);
	const bFileContent = fs.readFileSync(path.join(repoDir, "b_file"), "utf-8");
	expect(bFileContent).toContain("branch2 commit 1");

	// branch3 was excluded — c_file should be gone.
	expect(fs.existsSync(path.join(repoDir, "c_file"))).toBe(false);

	// At least one stack remaining (branch2 survived).
	await expect(getByTestId(page, "stack")).toHaveCount(1);
});

test("ref moved to different commit shows moved status and excluding removes its content", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// branch1 modifies a_file; we'll move its ref to a commit that also modifies
	// a_file differently, simulating an external force-push.
	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(1);

	const repoDir = path.join(workdir, "local-clone");
	const aFileBefore = fs.readFileSync(path.join(repoDir, "a_file"), "utf-8");
	expect(aFileBefore).toContain("branch1 commit 1");

	// Leave workspace.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);

	// Move the ref to a commit with different content.
	await gitbutler.runScript("move-stack-ref-to-conflicting-commit.sh", ["branch1", "local-clone"]);

	// Switch back — modal should show the divergence.
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });

	const divergenceItems = modal.getByTestId("workspace-divergence-modal-divergence-item");
	await expect(divergenceItems).toHaveCount(1);

	// The item should indicate the branch has moved.
	await expect(divergenceItems.first()).toContainText(/Moved/);

	// Explicitly select "Exclude" from the resolution dropdown.
	const selectTrigger = divergenceItems.first().getByRole("textbox");
	await selectTrigger.click();
	const excludeOption = page.getByRole("listbox").getByText("Exclude", { exact: true });
	await excludeOption.click();

	// Resolve and verify the external content is NOT in the workspace.
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");

	// After excluding, a_file should have the base content only — no branch content.
	const aFileAfter = fs.readFileSync(path.join(repoDir, "a_file"), "utf-8");
	expect(aFileAfter).not.toContain("branch1 commit 1");
	expect(aFileAfter).not.toContain("CONFLICTING CONTENT");
	await expect(getByTestId(page, "stack")).toHaveCount(0);
});

test("inter-stack conflict: moved ref conflicting with another stack defaults to exclude", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// branch1 modifies a_file, branch2 creates b_file — independent.
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	const repoDir = path.join(workdir, "local-clone");

	// branch1's changes should be present in a_file.
	expect(fs.readFileSync(path.join(repoDir, "a_file"), "utf-8")).toContain("branch1 commit 1");
	// branch2's b_file should exist.
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(true);

	// Leave workspace.
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	await assertBranch("master", repoDir);

	// Move branch2's ref to a commit that appends different content to a_file at the
	// same position where branch1 also appends. Both sides add at line 4, creating a
	// true 3-way merge conflict between the two stacks.
	await gitbutler.runScript("move-stack-ref-to-inter-stack-conflict.sh", [
		"branch2",
		"local-clone",
	]);

	// Switch back — divergence modal should appear.
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	const modal = page.getByTestId("workspace-divergence-modal");
	await modal.waitFor({ timeout: 30_000 });

	// At least one divergence item should mention conflict (branch2's inter-stack conflict).
	const hasConflictLabel = await modal.getByText(/conflict/i).count();
	expect(hasConflictLabel).toBeGreaterThan(0);

	// Resolve with defaults — conflicted stacks default to Exclude.
	await clickByTestId(page, "workspace-divergence-modal-action-button");
	await expect(modal).not.toBeVisible();
	await waitForTestId(page, "workspace-view");
	await assertBranch("gitbutler/workspace", repoDir);

	// branch1 should survive — its a_file content should be present.
	const aFileContent = fs.readFileSync(path.join(repoDir, "a_file"), "utf-8");
	expect(aFileContent).toContain("branch1 commit 1");
	// The conflicting content from the moved branch2 should NOT be present.
	expect(aFileContent).not.toContain("INTER-STACK CONFLICT LINE");
	// b_file should be gone — branch2's original content was excluded.
	expect(fs.existsSync(path.join(repoDir, "b_file"))).toBe(false);

	// At least one stack (branch1) should remain.
	await expect(getByTestId(page, "stack")).toHaveCount(1);
});
