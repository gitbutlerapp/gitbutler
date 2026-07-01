import { gotoOnboarding, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	getByTestId,
	mockPickDirectory,
	stack,
	waitForTestId,
} from "../src/util.ts";
import { expect } from "@playwright/test";

test("should handle gracefully adding an existing project", async ({ page, gitbutler }) => {
	const projectPath = gitbutler.pathInWorkdir("local-clone-2/");
	await gitbutler.runScript("two-projects-with-remote-branches.sh");
	await openWorkspace(page);

	await clickByTestId(page, "chrome-header-project-selector");
	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "chrome-header-project-selector-add-local-project");

	await waitForTestId(page, "add-project-already-exists-modal");
	// Click the modal first to dismiss the select dropdown behind it.
	await clickByTestId(page, "add-project-already-exists-modal", true);
	await clickByTestId(page, "add-project-already-exists-modal-open-project-button");

	await expect(getByTestId(page, "chrome-header-project-selector")).toContainText("local-clone-2");
});

test("should handle gracefully adding bare repo", async ({ page, gitbutler }) => {
	const projectPath = gitbutler.pathInWorkdir("local-clone/");
	await gitbutler.runScript("setup-empty-project-bare.sh");
	await gotoOnboarding(page);

	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	await waitForTestId(page, "add-project-bare-repo-modal");
});

test("should handle gracefully adding a non-git directory", async ({ page, gitbutler }) => {
	const projectPath = gitbutler.pathInWorkdir("not-a-git-repo/");
	await gitbutler.runScript("setup-empty-project.sh");
	await gotoOnboarding(page);

	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	await waitForTestId(page, "add-project-not-a-git-repo-modal");
});

test("should handle adding a project with extra commit and uncommitted changes on main branch", async ({
	page,
	gitbutler,
}) => {
	const projectPath = gitbutler.pathInWorkdir("local-with-changes/");
	await gitbutler.runScript("project-with-commit-and-uncommitted-changes.sh");
	await gotoOnboarding(page);

	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	clickByTestId(page, "set-base-branch");
	await waitForTestId(page, "workspace-view");

	const stacks = stack(page);
	await expect(stacks).toHaveCount(1);
	await expect(stacks.first()).not.toContainText("master");
});
