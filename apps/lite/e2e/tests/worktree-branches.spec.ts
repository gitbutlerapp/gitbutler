import { expect, test } from "../test.ts";

test.use({
	scenario: "project-in-single-branch-with-worktree.sh",
});

test("a worktree branch pushes with the branches it rests on", async ({ app }) => {
	await expect(app.branch("W")).toShow("Unpushed branch");
	await expect(
		app.branch("W").getByRole("button", { name: "Push this and all branches below", exact: true }),
	).toBeVisible();
});

test("the push key pushes a selected worktree branch", async ({ app }) => {
	await app.selectBranch("W");

	await app.press("Shift+P");

	await expect(app.branch("W")).toShow("Nothing to push");
});

test("a worktree branch renames in place", async ({ app }) => {
	const editor = await app.editBranchName("W");

	await editor.fill("W-renamed");
	await editor.press("Enter");

	await expect(app.branch("W-renamed")).toBeVisible();
});
