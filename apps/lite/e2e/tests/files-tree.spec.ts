import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "../test.ts";

test.describe("files tree", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("a directory row offers a menu and answers for the files below it", async ({
		appWindow,
		testEnvironment,
	}) => {
		const clone = path.join(testEnvironment.workdir, "local-clone");
		mkdirSync(path.join(clone, "src", "ui"), { recursive: true });
		writeFileSync(path.join(clone, "src", "ui", "row.txt"), "an uncommitted file\n");
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		const uncommittedFiles = appWindow.getByRole("tree", { name: "Uncommitted" });
		// A chain of directories holding nothing but the next one arrives as one row.
		const directory = uncommittedFiles.getByRole("treeitem", { name: "Directory src/ui" });
		await expect(directory).toHaveAttribute("aria-expanded", "true");

		// The toolbar shows itself for the selected row, as it does for a file.
		await directory.click();
		await expect(directory.getByRole("button", { name: "Directory menu" })).toBeVisible();

		// Checking the folder checks every file below it.
		await uncommittedFiles.getByRole("checkbox", { name: "Check directory src/ui" }).click();
		await expect(
			uncommittedFiles.getByRole("checkbox", { name: "Check file src/ui/row.txt" }),
		).toBeChecked();

		await uncommittedFiles.getByRole("button", { name: "Collapse directory src/ui" }).click();
		await expect(directory).toHaveAttribute("aria-expanded", "false");
		await expect(
			uncommittedFiles.getByRole("treeitem", { name: "Addition src/ui/row.txt" }),
		).toBeHidden();
	});
});
