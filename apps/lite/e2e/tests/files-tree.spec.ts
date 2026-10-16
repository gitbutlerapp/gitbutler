import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test.beforeEach(async ({ repo }) => {
	await repo.write("src/ui/row.txt");
});

test("a chain of directories holding nothing but the next one is one row", async ({ app }) => {
	await expect(app.uncommitted.getByRole("treeitem", { name: "Directory src/ui" })).toBeVisible();
});

test("a selected directory row offers a menu", async ({ app }) => {
	const directory = app.uncommitted.getByRole("treeitem", { name: "Directory src/ui" });

	await directory.click();

	await expect(directory.getByRole("button", { name: "Directory menu" })).toBeVisible();
});

test("checking a directory checks every file below it", async ({ app }) => {
	await app.uncommitted.getByRole("checkbox", { name: "Check directory src/ui" }).click();

	await expect(app.fileCheckbox("src/ui/row.txt")).toBeChecked();
});

test("collapsing a directory hides its files", async ({ app }) => {
	const directory = app.uncommitted.getByRole("treeitem", { name: "Directory src/ui" });

	await app.uncommitted.getByRole("button", { name: "Collapse directory src/ui" }).click();

	await expect(directory).toHaveAttribute("aria-expanded", "false");
	await expect(app.uncommittedFile("src/ui/row.txt")).toBeHidden();
});
