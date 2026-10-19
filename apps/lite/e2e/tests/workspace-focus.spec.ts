import type { Locator } from "@playwright/test";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

const backgroundOf = (locator: Locator) =>
	locator.evaluate((element) => getComputedStyle(element).backgroundColor);

test.describe("an uncommitted file next to a previewed commit", () => {
	test.beforeEach(async ({ app, repo }) => {
		await repo.write("added.txt");
		await app.uncommittedFile("added.txt").click();
		await app.selectCommit("C: first commit");
	});

	test("dragging it keeps the commit preview and its focus", async ({ app }) => {
		const file = app.uncommittedFile("added.txt");
		const idle = await backgroundOf(file);
		const box = await file.boundingBox();
		if (box === null) throw new Error("Uncommitted file row has no bounding box");

		await app.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await app.page.mouse.down();
		try {
			// Pressing to drag must not select the row.
			await expect.poll(() => backgroundOf(file)).toBe(idle);
			await app.page.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2, {
				steps: 10,
			});
		} finally {
			await app.page.mouse.up();
		}

		await expect.poll(() => app.activeList()).toBe("applied");
		await expect(app).toHaveFocusIn("sidebar");
	});

	test("checking it keeps the commit preview", async ({ app }) => {
		await app.fileCheckbox("added.txt").click();

		await expect.poll(() => app.activeList()).toBe("applied");
	});

	test("clicking it previews the file", async ({ app }) => {
		await app.uncommittedFile("added.txt").click();

		await expect.poll(() => app.activeList()).toBe("uncommitted");
		await expect(app).toHaveFocusIn("uncommitted-files");
	});
});

test.describe("a selected commit's details", () => {
	test.beforeEach(async ({ app }) => {
		await app.selectCommit("C: first commit");
		await app.showCommitFiles();
	});

	test("only the focused child uses focused selection styling", async ({ app }) => {
		const selectedFile = app.focusScope("files").locator('[role="treeitem"][aria-selected="true"]');
		await app.focusScope("diff").focus();
		await app.press("2");
		await expect(app.focusScope("sidebar").filter({ visible: true })).toBeFocused();
		let blurred = "";
		// Captured inside the poll so a virtualised row replacement cannot empty the baseline.
		await expect.poll(async () => (blurred = await backgroundOf(selectedFile))).not.toBe("");

		await app.focusScope("files").focus();
		await expect.poll(() => backgroundOf(selectedFile)).not.toBe(blurred);
		await app.focusScope("diff").focus();
		await expect.poll(() => backgroundOf(selectedFile)).toBe(blurred);
	});

	test("cancelled drags restore the committed panel's focus", async ({ app }) => {
		await app.focusScope("files").getByRole("treeitem").first().click();
		const nowhere = app.page.locator("body");

		for (const source of [
			app.focusScope("diff").locator('[draggable="true"]').first(),
			app.commit("C: first commit"),
		])
			await source.dragTo(nowhere, { targetPosition: { x: 1, y: 1 } });

		await expect(app).toHaveFocusIn("files");
	});
});

test("a dialog takes the selection's focus styling while it is open", async ({ app }) => {
	await app.selectCommit("C: first commit");
	const commit = await app.commit("C: first commit").elementHandle();
	if (commit === null) throw new Error("Selected commit row has no element");
	const focused = await commit.evaluate((element) => getComputedStyle(element).backgroundColor);
	const palette = app.getByRole("dialog", { name: "Command palette" });

	await app.press("ControlOrMeta+K");
	await expect(palette).toBeVisible();
	await expect(app.page.locator("[data-selection-focused]")).toHaveCount(0);
	await expect
		.poll(() => commit.evaluate((element) => getComputedStyle(element).backgroundColor))
		.not.toBe(focused);

	await app.press("Escape");
	await expect(palette).toBeHidden();
	await expect
		.poll(() => commit.evaluate((element) => getComputedStyle(element).backgroundColor))
		.toBe(focused);
});
