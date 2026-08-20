import path from "node:path";
import { LiteTestId } from "../../ui/src/testIds.ts";
import { expect, test } from "../test.ts";
import { assertHeadBranch } from "../utils.ts";

const initialBranch = "onboarding-test";

test.use({ scenario: "project-with-named-branch.sh" });

test("adds a local repository without changing its branch", async ({
	appWindow,
	electronApp,
	testEnvironment,
}) => {
	const repositoryPath = path.join(testEnvironment.workdir, "onboarding-repository");

	await electronApp.evaluate(({ dialog }, selectedPath) => {
		dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [selectedPath] });
	}, repositoryPath);

	await expect(appWindow.getByTestId(LiteTestId.OnboardingPage)).toBeVisible();
	await appWindow.getByRole("button", { name: "Add local repository" }).click();

	await expect(appWindow.getByTestId(/project=.*:workspace/)).toBeVisible();
	const projectPicker = appWindow.getByRole("button", { name: /Select project/ });
	await expect(projectPicker).toBeVisible();

	await projectPicker.click();
	await expect(appWindow.getByRole("option", { name: /onboarding-repository/i })).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Add local repository" })).toBeVisible();

	assertHeadBranch(repositoryPath, initialBranch);
});
