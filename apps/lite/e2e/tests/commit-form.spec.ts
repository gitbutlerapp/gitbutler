import { writeFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "../test.ts";

test.describe("commit form", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("Escape closes the form and keeps the checked files", async ({
		appWindow,
		testEnvironment,
	}) => {
		const clone = path.join(testEnvironment.workdir, "local-clone");
		writeFileSync(path.join(clone, "added.txt"), "an uncommitted file\n");
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		const uncommittedFiles = appWindow.getByRole("tree", { name: "Uncommitted" });
		const checkbox = uncommittedFiles.getByRole("checkbox", { name: "Check file added.txt" });
		await checkbox.click();
		await expect(checkbox).toBeChecked();

		const startCommit = appWindow.getByRole("button", { name: /start commit/i });
		await startCommit.click();

		const message = appWindow.getByRole("textbox", { name: "Compose commit message" });
		await expect(message).toBeFocused();
		await appWindow.keyboard.press("Escape");

		// The form is the only thing Escape was asked to close. The selection it was opened to
		// commit outlives it, so reopening doesn't mean re-checking everything.
		await expect(startCommit).toBeVisible();
		await expect(checkbox).toBeChecked();
	});
});
