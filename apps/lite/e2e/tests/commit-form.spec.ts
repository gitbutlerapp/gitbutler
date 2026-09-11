import { writeFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "../test.ts";

test.describe("commit form", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("Escape closes the form and keeps the checked files", async ({
		appWindow,
		testEnvironment,
	}) => {
		const clone = path.join(testEnvironment.workdir, "local-clone");
		writeFileSync(path.join(clone, "added.txt"), "an uncommitted file\n");
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		const uncommittedFiles = appWindow.getByRole("tree", { name: "Uncommitted" });
		const checkbox = uncommittedFiles.getByRole("checkbox", { name: "Check file added.txt" });
		await checkbox.click();
		await expect(checkbox).toBeChecked();

		const startCommit = appWindow.getByRole("button", { name: /start commit/i });
		await startCommit.click();

		const message = appWindow.getByRole("textbox", { name: "Compose commit message" });
		await expect(message).toBeFocused();
		await appWindow.keyboard.press("Escape");

		// The form is the only thing Escape was asked to close. The selection it was opened to
		// commit outlives it, so reopening doesn't mean re-checking everything.
		await expect(startCommit).toBeVisible();
		await expect(checkbox).toBeChecked();
	});
});

test.describe("commit split view", () => {
	test.use({ scenario: "project-with-many-independent-stacks.sh" });
	test("keeps the button in place, scrolls each pane independently, and preserves the draft when resized", async ({
		appWindow,
		testEnvironment,
	}) => {
		test.slow();
		const clone = path.join(testEnvironment.workdir, "local-clone");
		for (let index = 0; index < 80; index++) {
			writeFileSync(
				path.join(clone, `uncommitted-${String(index).padStart(3, "0")}.txt`),
				"new file\n",
			);
		}
		await appWindow.reload();
		const start = appWindow.getByRole("button", { name: /start commit/i });
		await expect(start).toBeInViewport();
		const shared = appWindow.locator('[data-commit-mode="false"]');
		const upstream = appWindow.locator('[class*="docked"]');
		const expectCommitDocked = async () => {
			await expect
				.poll(async () => {
					const footerElement = appWindow.locator('[class*="commitRow"]');
					const footer = await footerElement.boundingBox();
					const rail = await footerElement.locator('[class*="stretchSegment"]').boundingBox();
					const button = await start.boundingBox();
					const row = await upstream.boundingBox();
					if (!footer || !rail || !button || !row) return Infinity;
					const bottom = footer.y + footer.height;
					return Math.max(
						Math.abs(row.y - bottom),
						Math.abs(rail.y + rail.height - bottom),
						Math.abs(button.y - footer.y - (bottom - button.y - button.height)),
					);
				})
				.toBeLessThan(1);
		};
		const expectUpstreamDocked = async () => {
			await expect(upstream).toBeVisible();
			await expect(upstream).toBeInViewport({ ratio: 1 });
			await expect
				.poll(async () => {
					const row = await upstream.boundingBox();
					const pane = await appWindow.locator("#sidebar-panel").boundingBox();
					return row && pane ? Math.abs(row.y + row.height - pane.y - pane.height) : Infinity;
				})
				.toBeLessThan(1);
		};
		await expectUpstreamDocked();
		await expectCommitDocked();
		await shared.evaluate((el) => {
			el.scrollTop = 400;
		});
		await expect.poll(() => shared.evaluate((el) => el.scrollTop)).toBe(400);
		await expectUpstreamDocked();
		await expectCommitDocked();
		const before = await start.boundingBox();
		if (!before) throw new Error("Commit button has no box");
		await start.click();
		const message = appWindow.getByRole("textbox", { name: "Compose commit message" });
		await expect(message).toBeFocused();
		const submit = appWindow.locator('[data-commit-form] button[type="submit"]');
		const expectComposerInset = async () => {
			await expect
				.poll(async () => {
					const row = await appWindow.locator('[class*="commitRow"]').boundingBox();
					const form = await appWindow.locator("[data-commit-form]").boundingBox();
					return row && form ? row.y + row.height - form.y - form.height : 0;
				})
				.toBe(8);
		};
		await expectComposerInset();
		await expect
			.poll(async () => {
				const box = await submit.boundingBox();
				return box ? Math.abs(box.y + box.height - before.y - before.height) : Infinity;
			})
			.toBeLessThan(2);
		await message.fill("Keep this draft while resizing");
		const divider = appWindow.getByRole("separator", {
			name: "Resize uncommitted files and workspace",
		});
		const dividerBox = await divider.boundingBox();
		if (!dividerBox) throw new Error("Split view has no divider");
		await appWindow.mouse.move(
			dividerBox.x + dividerBox.width / 2,
			dividerBox.y + dividerBox.height / 2,
		);
		await appWindow.mouse.down();
		await appWindow.mouse.move(dividerBox.x + dividerBox.width / 2, dividerBox.y - 220, {
			steps: 8,
		});
		await appWindow.mouse.up();
		await expect
			.poll(async () => (await divider.boundingBox())?.y ?? Infinity)
			.toBeLessThan(dividerBox.y - 150);
		await expect(message).toHaveValue("Keep this draft while resizing");
		await expectComposerInset();
		await expectUpstreamDocked();
		const files = appWindow.locator("[data-uncommitted-scroll]").filter({ visible: true });
		const stacks = appWindow.locator('#commit-stacks [class*="workspaceScroller"]');
		const fileOffset = await files.evaluate((el) => el.scrollTop);
		const submitBefore = await submit.boundingBox();
		const stackBox = await stacks.boundingBox();
		if (!stackBox || !submitBefore) throw new Error("Missing split region");
		await appWindow.mouse.move(stackBox.x + stackBox.width / 2, stackBox.y + stackBox.height / 2);
		await appWindow.mouse.wheel(0, 900);
		await expect.poll(() => stacks.evaluate((el) => el.scrollTop)).toBeGreaterThan(300);
		expect(await files.evaluate((el) => el.scrollTop)).toBe(fileOffset);
		const stackOffset = await stacks.evaluate((el) => el.scrollTop);
		const fileBox = await files.boundingBox();
		if (!fileBox) throw new Error("Missing file region");
		await appWindow.mouse.move(fileBox.x + fileBox.width / 2, fileBox.y + fileBox.height / 2);
		await appWindow.mouse.wheel(0, 500);
		await expect.poll(() => files.evaluate((el) => el.scrollTop)).toBeGreaterThan(fileOffset);
		expect(await stacks.evaluate((el) => el.scrollTop)).toBe(stackOffset);
		expect((await submit.boundingBox())?.y).toBeCloseTo(submitBefore.y, 0);
		const cancelOffset = await files.evaluate((el) => el.scrollTop);
		const anchor = await files.evaluate((el) => {
			const top = el.getBoundingClientRect().top;
			const row = [...el.querySelectorAll('[role="treeitem"]')].find(
				(row) => row.getBoundingClientRect().top >= top,
			);
			if (!row) throw new Error("No visible file to anchor cancellation");
			const name = row.getAttribute("aria-label");
			if (name === null) throw new Error("File row has no label");
			return { name, top: row.getBoundingClientRect().top };
		});
		await message.focus();
		await appWindow.keyboard.press("Escape");
		await expect(divider).toHaveCount(0);
		await expect.poll(() => shared.evaluate((el) => el.scrollTop)).toBe(cancelOffset);
		await expectUpstreamDocked();
		await expectCommitDocked();
		await expect
			.poll(async () => {
				const box = await appWindow
					.getByRole("treeitem", { name: anchor.name, exact: true })
					.boundingBox();
				return box ? Math.abs(box.y - anchor.top) : Infinity;
			})
			.toBeLessThan(2);
		await start.click();
		await expect(message).toHaveValue("Keep this draft while resizing");
		const longDraft = "A longer commit message\n".repeat(40);
		const buttonBox = await submit.boundingBox();
		if (!buttonBox) throw new Error("Commit button is missing");
		const buttonTop = buttonBox.y;
		await message.fill(longDraft);
		await expect(submit).toBeInViewport();
		await expect
			.poll(async () => Math.abs(((await submit.boundingBox())?.y ?? Infinity) - buttonTop))
			.toBeLessThan(2);
		const split = await divider.boundingBox();
		const workspace = await stacks.boundingBox();
		if (!split || !workspace) throw new Error("Missing split region");
		await appWindow.mouse.move(split.x + split.width / 2, split.y + split.height / 2);
		await appWindow.mouse.down();
		await appWindow.mouse.move(split.x + split.width / 2, workspace.y + workspace.height - 1);
		await appWindow.mouse.up();
		await expect
			.poll(async () => (await stacks.boundingBox())?.height ?? Infinity)
			.toBeLessThan(40);
		await expect
			.poll(async () => {
				const row = await upstream.boundingBox();
				const pane = await stacks.boundingBox();
				return row && pane ? row.y - pane.y : -Infinity;
			})
			.toBeGreaterThanOrEqual(0);
		await message.focus();
		await appWindow.keyboard.press("Escape");
		await expect(divider).toHaveCount(0);
		await shared.evaluate((el) => {
			el.scrollTop = el.scrollHeight;
		});
		await expect(upstream).toBeHidden();
		await expect(
			appWindow.getByText("origin/master", { exact: true }).filter({ visible: true }),
		).toBeInViewport();
	});
});
