import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("Escape closes the commit form and keeps the checked files", async ({ app, repo }) => {
	await repo.write("added.txt");
	await app.fileCheckbox("added.txt").click();
	await app.startCommit();

	await app.press("Escape");

	await expect(app.startCommitButton).toBeVisible();
	await expect(app.fileCheckbox("added.txt")).toBeChecked();
});
