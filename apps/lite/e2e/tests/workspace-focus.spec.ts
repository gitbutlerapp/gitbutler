import { writeFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "../test.ts";

test.describe("workspace focus", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("dragging an uncommitted file retains the previewing panel's focus", async ({
		appWindow,
		testEnvironment,
	}) => {
		const clone = path.join(testEnvironment.workdir, "local-clone");
		writeFileSync(path.join(clone, "added.txt"), "an uncommitted file\n");
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		const uncommittedFiles = appWindow.getByRole("tree", { name: "Uncommitted" });
		const file = uncommittedFiles.getByRole("treeitem", { name: "Addition added.txt" });
		await file.click();

		const commit = appWindow.getByRole("treeitem", { name: "C: first commit" });
		await commit.click();
		await expect(commit).toHaveAttribute("aria-selected", "true");
		await expect
			.poll(() =>
				appWindow.evaluate(() =>
					document.activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope"),
				),
			)
			.toBe("sidebar");

		const box = await file.boundingBox();
		if (box === null) throw new Error("Uncommitted file row has no bounding box");

		await appWindow.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		const selectionColor = await file.evaluate(
			(element) => getComputedStyle(element).backgroundColor,
		);
		await appWindow.mouse.down();
		try {
			await expect
				.poll(() => file.evaluate((element) => getComputedStyle(element).backgroundColor))
				.toBe(selectionColor);
			await appWindow.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2, {
				steps: 10,
			});
		} finally {
			await appWindow.mouse.up();
		}

		await expect.poll(() => new URL(appWindow.url()).searchParams.get("active")).toBeNull();
		await expect
			.poll(() =>
				appWindow.evaluate(() =>
					document.activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope"),
				),
			)
			.toBe("sidebar");

		await uncommittedFiles.getByRole("checkbox", { name: "Check file added.txt" }).click();
		await expect.poll(() => new URL(appWindow.url()).searchParams.get("active")).toBeNull();

		await file.click();
		await expect
			.poll(() => new URL(appWindow.url()).searchParams.get("active"))
			.toBe("uncommitted");
		await expect
			.poll(() =>
				appWindow.evaluate(() =>
					document.activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope"),
				),
			)
			.toBe("uncommitted-files");
	});

	test("only the focused details child uses focused selection styling", async ({ appWindow }) => {
		await appWindow.getByRole("treeitem", { name: "C: first commit" }).click();

		const toggleFiles = appWindow.getByRole("button", { name: "Toggle files" });
		if ((await toggleFiles.getAttribute("aria-pressed")) !== "true") await toggleFiles.click();

		const sidebar = appWindow.locator('[data-focus-scope="sidebar"]');
		const files = appWindow.locator('[data-focus-scope="files"]');
		const diff = appWindow.locator('[data-focus-scope="diff"]');
		const selectedFile = files.locator('[role="treeitem"][aria-selected="true"]');
		await expect(selectedFile).toBeVisible();

		const selectedFileBackground = () =>
			selectedFile.evaluate((element) => getComputedStyle(element).backgroundColor);

		await sidebar.focus();
		// A virtualized row can be replaced between visibility and style checks, briefly yielding no computed style.
		await expect.poll(selectedFileBackground).not.toBe("");
		const blurredBackground = await selectedFileBackground();
		await files.focus();
		await expect.poll(selectedFileBackground).not.toBe(blurredBackground);
		await diff.focus();
		await expect.poll(selectedFileBackground).toBe(blurredBackground);
	});

	test("cancelled drags restore the committed panel focus", async ({ appWindow }) => {
		const commit = appWindow.getByRole("treeitem", { name: "C: first commit" });
		await commit.click();

		const toggleFiles = appWindow.getByRole("button", { name: "Toggle files" });
		if ((await toggleFiles.getAttribute("aria-pressed")) !== "true") await toggleFiles.click();

		const files = appWindow.locator('[data-focus-scope="files"]');
		await files.locator('[role="treeitem"]').first().click();

		const nowhere = appWindow.locator("body");
		const dragAndCancel = (source: typeof commit) =>
			source.dragTo(nowhere, { targetPosition: { x: 1, y: 1 } });

		const diffFileHeader = appWindow
			.locator('[data-focus-scope="diff"] [draggable="true"]')
			.first();
		await dragAndCancel(diffFileHeader);
		await dragAndCancel(commit);

		await expect
			.poll(() =>
				appWindow.evaluate(() =>
					document.activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope"),
				),
			)
			.toBe("files");
	});

	test("clears selection focus while a dialog owns focus", async ({ appWindow }) => {
		const commit = appWindow.getByRole("treeitem", { name: "C: first commit" });
		await commit.click();
		const commitElement = await commit.elementHandle();
		if (commitElement === null) throw new Error("Selected commit row has no element");
		const commitBackground = () =>
			commitElement.evaluate((element) => getComputedStyle(element).backgroundColor);
		const focusedBackground = await commitBackground();

		await appWindow.keyboard.press("ControlOrMeta+K");
		await expect(appWindow.getByRole("dialog", { name: "Command palette" })).toBeVisible();
		await expect(appWindow.locator("[data-selection-focused]")).toHaveCount(0);
		await expect.poll(commitBackground).not.toBe(focusedBackground);

		await appWindow.keyboard.press("Escape");
		await expect(appWindow.getByRole("dialog", { name: "Command palette" })).not.toBeVisible();
		await expect.poll(commitBackground).toBe(focusedBackground);
	});
});
