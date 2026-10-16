import path from "node:path";
import { expect, test } from "../test.ts";
import { assertHeadBranch } from "../utils.ts";

test.use({ scenario: "project-with-named-branch.sh" });

test("adds a local repository without changing its branch", async ({ app, testEnvironment }) => {
	const repositoryPath = path.join(testEnvironment.workdir, "onboarding-repository");
	await app.answerOpenDialogsWith(repositoryPath);

	await app.getByRole("button", { name: "Add local repository" }).click();

	await app.projectPicker.click();
	await expect(app.getByRole("option", { name: /onboarding-repository/i })).toBeVisible();
	assertHeadBranch(repositoryPath, "onboarding-test");
});
