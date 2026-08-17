import path from "node:path";
import { LiteTestId } from "@gitbutler/ui/utils/testIds";
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
	await expect(appWindow.getByTestId(LiteTestId.ProjectPickerButton)).toContainText("local-clone");

	await appWindow.getByTestId(LiteTestId.ProjectPickerButton).click();
	await appWindow.getByTestId(LiteTestId.ProjectPickerAddLocalProjectButton).click();

	await expect(appWindow.getByTestId(LiteTestId.ProjectPickerButton)).toContainText(
		"additional-repository",
	);
});
