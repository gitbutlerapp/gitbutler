import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-with-worktree.sh" });

test("a worktree branch pushes with the branches it rests on", async ({ appWindow }) => {
	const branch = appWindow.getByRole("treeitem", { name: "W", exact: true });
	await expect(branch.getByText("Unpushed branch", { exact: true })).toBeVisible();
	await expect(
		branch.getByRole("button", { name: "Push this and all branches below", exact: true }),
	).toBeVisible();
});

test("the push key pushes a selected worktree branch", async ({ appWindow }) => {
	const branch = appWindow.getByRole("treeitem", { name: "W", exact: true });
	await branch.click();
	await expect(branch).toHaveAttribute("aria-selected", "true");

	await appWindow.keyboard.press("Shift+P");

	await expect(branch.getByText("Nothing to push", { exact: true })).toBeVisible();
});

test("a worktree branch renames in place", async ({ appWindow }) => {
	await appWindow.getByRole("treeitem", { name: "W", exact: true }).click();
	await appWindow.keyboard.press("F2");
	const editor = appWindow.getByRole("textbox", { name: "Branch name" });
	await editor.fill("W-renamed");
	await editor.press("Enter");
	await expect(appWindow.getByRole("treeitem", { name: "W-renamed", exact: true })).toBeVisible();
});
