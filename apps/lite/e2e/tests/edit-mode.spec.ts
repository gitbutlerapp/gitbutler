import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "../test.ts";

/**
 * The slice of the preload bridge these tests drive directly. A clean commit
 * only offers edit mode through a native context menu, which Playwright cannot
 * open, so those tests call what the menu item calls. The conflicted tests go
 * through the conflict bar's button instead, which is a real one.
 */
type LiteBridge = {
	headInfo: (projectId: string) => Promise<{
		stacks: Array<{ id: string; segments: Array<{ commits: Array<{ id: string }> }> }>;
	}>;
	enterEditMode: (payload: {
		projectId: string;
		commitId: string;
		stackId: string;
	}) => Promise<unknown>;
};

const editingHeading = (appWindow: Page) =>
	appWindow.getByRole("heading", { name: "Editing commit" });

/** Check out the workspace's only commit, the way the commit menu does. */
const enterEditModeThroughBridge = async (appWindow: Page): Promise<void> => {
	await appWindow.evaluate(async () => {
		const lite = (window as unknown as { lite: LiteBridge }).lite;
		const projectId = location.pathname.split("/")[2];
		if (projectId === undefined) throw new Error("No project in the URL");

		const { stacks } = await lite.headInfo(projectId);
		const stack = stacks[0];
		const commit = stack?.segments.flatMap((segment) => segment.commits)[0];
		if (!stack || !commit) throw new Error("The seeded project has no commit to edit");

		await lite.enterEditMode({ projectId, commitId: commit.id, stackId: stack.id });
	});
	await expect(editingHeading(appWindow)).toBeVisible();
};

/**
 * Open edit mode the way a user meets it: select the commit that could not be
 * applied, then take the conflict bar's offer. The bar lives in the commit's
 * details, so nothing appears until the commit is selected.
 */
const openEditModeFromConflictBar = async (appWindow: Page): Promise<void> => {
	await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace/);
	await appWindow
		.getByRole("treeitem", { name: /Change juliet locally/ })
		.first()
		.click();

	await expect(appWindow.getByText(/could not be applied/)).toBeVisible();
	await appWindow.getByRole("button", { name: "Open Edit Mode" }).click();
	await expect(editingHeading(appWindow)).toBeVisible();
};

const conflictedFilePath = (workdir: string): string => path.join(workdir, "local-clone", "a_file");

/** Resolve the file the way an editor would: drop every marker line. */
const removeConflictMarkers = (file: string): void => {
	writeFileSync(
		file,
		readFileSync(file, "utf8")
			.split("\n")
			.filter((line) => !/^(<{7}|\|{7}|={7}|>{7})/.test(line))
			.join("\n"),
	);
};

test.describe("edit mode without conflicts", () => {
	test.use({ scenario: "project-with-editable-commit.sh" });

	test("lists the commit's files and leaves the workspace on cancel", async ({ appWindow }) => {
		await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace/);
		await enterEditModeThroughBridge(appWindow);

		await expect(appWindow.getByText("a_file")).toBeVisible();
		await expect(appWindow.getByText("No changes yet.")).toBeVisible();

		// Nothing is conflicted, so the page offers no way to open conflicts.
		await expect(appWindow.getByRole("button", { name: /conflicted file/i })).toHaveCount(0);

		await appWindow.getByRole("button", { name: "Cancel edit" }).click();
		await expect(editingHeading(appWindow)).toBeHidden();
	});
});

test.describe("edit mode with conflicts", () => {
	test.use({ scenario: "project-with-conflicted-commit.sh" });

	test("opens from the conflict bar on the commit that cannot apply", async ({ appWindow }) => {
		// The bar is the only place that offers edit mode without a native menu,
		// and it says the commit needs it, so it has to lead there.
		await openEditModeFromConflictBar(appWindow);
		// The bar led to this commit's own page, conflict and all.
		await expect(appWindow.getByText("conflicts", { exact: true })).toBeVisible();
	});

	test("marks a conflicted file resolved once its markers are gone", async ({
		appWindow,
		testEnvironment,
	}) => {
		await enterEditModeThroughBridge(appWindow);

		// The hint is matched exactly: the page's explainer says "conflicts" too.
		// The commit could not be applied, so its file is checked out with markers.
		await expect(appWindow.getByText("conflicts", { exact: true })).toBeVisible();
		await expect(appWindow.getByRole("button", { name: /conflicted file/i })).toBeVisible();

		const conflictedFile = conflictedFilePath(testEnvironment.workdir);
		expect(readFileSync(conflictedFile, "utf8")).toContain("<<<<<<<");

		// The state is read from disk, so this is what the page has to notice.
		removeConflictMarkers(conflictedFile);

		await expect(appWindow.getByText("resolved", { exact: true })).toBeVisible();
		await expect(appWindow.getByText("conflicts", { exact: true })).toHaveCount(0);
	});

	test("saving a resolved commit rewrites it and clears the conflict", async ({
		appWindow,
		testEnvironment,
	}) => {
		await openEditModeFromConflictBar(appWindow);

		const conflictedFile = conflictedFilePath(testEnvironment.workdir);
		removeConflictMarkers(conflictedFile);
		await expect(appWindow.getByText("resolved", { exact: true })).toBeVisible();

		await appWindow.getByRole("button", { name: "Save and return" }).click();

		// Back in the workspace, with nothing left to resolve.
		await expect(editingHeading(appWindow)).toBeHidden();
		await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace/);
		await expect(appWindow.getByRole("button", { name: "Open Edit Mode" })).toHaveCount(0);
		expect(readFileSync(conflictedFile, "utf8")).not.toContain("<<<<<<<");
	});

	test("refuses to save while the conflict markers are still there", async ({ appWindow }) => {
		await openEditModeFromConflictBar(appWindow);

		// Declining is the cautious answer: the save is abandoned and edit mode
		// stays open with the commit intact. The listener must answer the dialog
		// itself — registering one turns off Playwright's auto-dismiss, and an
		// unanswered `confirm` blocks the renderer.
		const asked = new Promise<string>((resolve) => {
			appWindow.once("dialog", (dialog) => {
				resolve(dialog.message());
				void dialog.dismiss();
			});
		});
		await appWindow.getByRole("button", { name: "Save and return" }).click();

		expect(await asked).toMatch(/not resolved/);
		await expect(editingHeading(appWindow)).toBeVisible();
	});
});
