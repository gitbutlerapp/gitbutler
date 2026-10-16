import path from "node:path";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-additional-repository.sh" });

test("navigates to a project added from the project picker", async ({ app, testEnvironment }) => {
	await app.answerOpenDialogsWith(path.join(testEnvironment.workdir, "additional-repository"));

	await app.projectPicker.click();
	await app.getByRole("button", { name: "Add local repository" }).click();

	await expect(app.projectPicker).toContainText("additional-repository");
});
