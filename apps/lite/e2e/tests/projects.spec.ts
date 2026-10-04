import path from "node:path";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-additional-repository.sh" });

test("navigates to a project added from the sidebar header", async ({
	appWindow,
	electronApp,
	testEnvironment,
}) => {
	const repositoryPath = path.join(testEnvironment.workdir, "additional-repository");

	await electronApp.evaluate(({ dialog }, selectedPath) => {
		dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [selectedPath] });
	}, repositoryPath);

	await expect(appWindow.getByTestId(/project=.*:workspace/)).toBeVisible();
	// Grouped by repo, the open project is badged as such.
	await appWindow.getByRole("button", { name: "Repos", exact: true }).click();
	await expect(appWindow.getByRole("treeitem", { name: /local-clone.*Open/ })).toBeVisible();

	await appWindow.getByRole("button", { name: "Add local repository" }).click();

	await expect(
		appWindow.getByRole("treeitem", { name: /additional-repository.*Open/ }),
	).toBeVisible();
});
