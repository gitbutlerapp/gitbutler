import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

/**
 * Open edit mode the way a user meets it: select the commit that could not be
 * applied, then take the conflict bar's offer.
 */
const openEditModeFromConflictBar = async (app: App): Promise<void> => {
	await app
		.getByRole("treeitem", { name: /Change juliet locally/ })
		.first()
		.click();
	await app.getByRole("button", { name: "Open Edit Mode" }).click();
	await expect(app.editModeHeading).toBeVisible();
};

// Matched exactly: the page's explainer says "conflicts" too.
const fileState = (app: App, state: "conflicts" | "resolved") =>
	app.getByText(state, { exact: true });

test.describe("edit mode without conflicts", () => {
	test.use({ scenario: "project-with-editable-commit.sh" });

	test("lists the commit's files without conflicts", async ({ app }) => {
		await app.enterEditMode();

		await expect(app.getByText("a_file")).toBeVisible();
		await expect(app.getByText("No changes yet.")).toBeVisible();
		await expect(fileState(app, "conflicts")).toHaveCount(0);
	});

	test("cancelling leaves edit mode", async ({ app }) => {
		await app.enterEditMode();

		await app.getByRole("button", { name: "Cancel edit" }).click();

		await expect(app.editModeHeading).toBeHidden();
	});
});

test.describe("edit mode with conflicts", () => {
	test.use({ scenario: "project-with-conflicted-commit.sh" });

	test("opens from the conflict bar on the commit that cannot apply", async ({ app }) => {
		await openEditModeFromConflictBar(app);

		await expect(fileState(app, "conflicts")).toBeVisible();
	});

	test("marks a conflicted file resolved once its markers are gone", async ({ app, repo }) => {
		await app.enterEditMode();
		await expect(fileState(app, "conflicts")).toBeVisible();

		repo.resolveConflictMarkers("a_file");

		await expect(fileState(app, "resolved")).toBeVisible();
		await expect(fileState(app, "conflicts")).toHaveCount(0);
	});

	test("saving a resolved commit rewrites it and clears the conflict", async ({ app, repo }) => {
		await openEditModeFromConflictBar(app);
		repo.resolveConflictMarkers("a_file");
		await expect(fileState(app, "resolved")).toBeVisible();

		await app.getByRole("button", { name: "Save and return" }).click();

		await expect(app.editModeHeading).toBeHidden();
		await expect(app.getByRole("button", { name: "Open Edit Mode" })).toHaveCount(0);
		expect(repo.read("a_file")).not.toContain("<<<<<<<");
	});

	test("refuses to save while the conflict markers are still there", async ({ app }) => {
		await openEditModeFromConflictBar(app);
		// Declining abandons the save. The listener must answer the dialog itself:
		// registering one turns off Playwright's auto-dismiss, and an unanswered
		// `confirm` blocks the renderer.
		const asked = new Promise<string>((resolve) => {
			app.page.once("dialog", (dialog) => {
				resolve(dialog.message());
				void dialog.dismiss();
			});
		});

		await app.getByRole("button", { name: "Save and return" }).click();

		expect(await asked).toMatch(/not resolved/);
		await expect(app.editModeHeading).toBeVisible();
	});
});
