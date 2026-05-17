import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, waitForTestId, waitForTestIdToNotExist } from "../src/util.ts";

async function openProjectSettingsAndDelete(page: import("@playwright/test").Page) {
	await clickByTestId(page, "chrome-sidebar-project-settings-button");
	await waitForTestId(page, "project-settings-modal");

	const deleteButton = await waitForTestId(page, "project-delete-button");
	await deleteButton.scrollIntoViewIfNeeded();
	await deleteButton.click();
	await clickByTestId(page, "project-delete-modal-confirm");

	await waitForTestIdToNotExist(page, "project-delete-modal-confirm");
	await waitForTestIdToNotExist(page, "project-delete-button");
	await waitForTestIdToNotExist(page, "project-settings-modal");
}

test("should be able to delete the last project gracefully", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);
	await openProjectSettingsAndDelete(page);
	await waitForTestId(page, "welcome-page");
});

test("should be able to delete a project when multiple exist", async ({ page, gitbutler }) => {
	await gitbutler.runScript("two-projects-with-remote-branches.sh");
	await openWorkspace(page);
	await openProjectSettingsAndDelete(page);
	await waitForTestId(page, "workspace-view");
});
