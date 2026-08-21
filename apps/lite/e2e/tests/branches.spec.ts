import { expect, test } from "../test.ts";

test.describe("branches", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("applies a remote branch to the workspace", async ({ appWindow }) => {
		await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();

		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		const secondCommit = appWindow.getByRole("treeitem", { name: "branch1: second commit" });
		const firstCommit = appWindow.getByRole("treeitem", { name: "branch1: first commit" });
		await expect(branch).toHaveCount(0);
		await expect(secondCommit).toHaveCount(0);
		await expect(firstCommit).toHaveCount(0);

		const picker = appWindow.getByRole("dialog", { name: "Apply branch" });
		// Hotkeys register in a passive effect, so retry the keypress if the workspace renders first.
		await expect(async () => {
			await appWindow.keyboard.press("ControlOrMeta+Shift+A");
			await expect(picker).toBeVisible({ timeout: 500 });
		}).toPass({ timeout: 5_000 });

		const search = picker.getByRole("combobox", { name: /search for branches/i });
		await search.fill("branch1");
		await picker.getByRole("option", { name: /^branch1 / }).click();

		await expect(picker).toBeHidden();
		await expect(branch).toBeVisible();
		await expect(secondCommit).toBeVisible();
		await expect(firstCommit).toBeVisible();
	});
});
