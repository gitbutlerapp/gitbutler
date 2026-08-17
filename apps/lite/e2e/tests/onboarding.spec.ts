import path from "node:path";
import { LiteTestId } from "@gitbutler/ui/utils/testIds";
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
	await appWindow.getByTestId(LiteTestId.OnboardingAddLocalProjectButton).click();

	await expect(appWindow.getByTestId(/project=.*:workspace/)).toBeVisible();
	await expect(appWindow.getByTestId(LiteTestId.ProjectPickerButton)).toBeVisible();

	await appWindow.getByTestId(LiteTestId.ProjectPickerButton).click();
	await expect(appWindow.getByRole("option", { name: /onboarding-repository/i })).toBeVisible();
	await expect(appWindow.getByTestId(LiteTestId.ProjectPickerAddLocalProjectButton)).toBeVisible();

	assertHeadBranch(repositoryPath, initialBranch);
});
