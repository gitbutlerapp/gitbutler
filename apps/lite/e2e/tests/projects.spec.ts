import path from "node:path";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-additional-repository.sh" });

test("navigates to a project added from the project picker", async ({
	appWindow,
	electronApp,
	testEnvironment,
}) => {
	const repositoryPath = path.join(testEnvironment.workdir, "additional-repository");

	await electronApp.evaluate(({ dialog }, selectedPath) => {
		dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [selectedPath] });
	}, repositoryPath);

	await expect(appWindow.getByTestId(/project=.*:workspace/)).toBeVisible();
	const projectPicker = appWindow.getByRole("combobox", { name: /Select project/ });
	await expect(projectPicker).toContainText("local-clone");

	await projectPicker.click();
	await appWindow.getByRole("button", { name: "Add local repository" }).click();

	await expect(projectPicker).toContainText("additional-repository");
});
