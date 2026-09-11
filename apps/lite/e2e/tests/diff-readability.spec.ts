import { writeFileSync } from "node:fs";
import path from "node:path";
import { enabled, openProject, shoot } from "../screenshot-helpers.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("reads both sides of a replacement, wraps and scrolls long lines, and tracks review progress", async ({
	appWindow,
	testEnvironment,
}) => {
	await openProject(appWindow);
	const longLine = `const value = "${"long-token-".repeat(100)}";`;
	writeFileSync(
		path.join(testEnvironment.workdir, "local-clone", "base.txt"),
		`${longLine}\nsecond line\n`,
	);
	await appWindow.reload();
	await appWindow
		.locator("#sidebar-panel")
		.getByRole("treeitem", { name: "Modification base.txt", exact: true })
		.click();
	const details = appWindow.locator("#details-panel");
	await details.getByRole("button", { name: "Unified", exact: true }).click();
	const oldNumbers = details.locator('[data-gitbutler-line-number="old"]');
	const newNumbers = details.locator('[data-gitbutler-line-number="new"]');
	await expect(oldNumbers).toHaveText(["1", "·", "·"]);
	await expect(newNumbers).toHaveText(["·", "1", "2"]);
	const line = details.locator('[data-line-type="change-addition"][data-line]').first();
	await expect(line).toHaveText(longLine);
	await expect(line).toHaveCSS("white-space", "pre-wrap");
	await expect
		.poll(() => line.evaluate((el) => el.getBoundingClientRect().height))
		.toBeGreaterThan(40);
	await expect(line).toHaveCSS("overflow-wrap", "anywhere");
	await details.getByRole("button", { name: "Scroll", exact: true }).click();
	await expect(line).toHaveCSS("white-space", "pre");
	expect(
		await line.evaluate((el) => {
			let ancestor = el.parentElement;
			while (ancestor && ancestor.scrollWidth <= ancestor.clientWidth)
				ancestor = ancestor.parentElement;
			if (!ancestor) return false;
			ancestor.scrollLeft = ancestor.scrollWidth;
			return ancestor.scrollLeft > 0;
		}),
	).toBe(true);
	await appWindow.reload();
	await expect(details.getByRole("button", { name: "Scroll", exact: true })).toHaveAttribute(
		"aria-pressed",
		"true",
	);
	const progress = details.getByRole("progressbar", { name: "Files reviewed" });
	await expect(progress).toHaveAttribute("aria-valuenow", "0");
	await details.getByRole("button", { name: "Mark all reviewed", exact: true }).click();
	await expect(progress).toHaveAttribute("aria-valuenow", "1");
	await details.getByRole("button", { name: "Reviewed", exact: true }).click();
	await expect(progress).toHaveAttribute("aria-valuenow", "0");
	if (enabled) {
		await details.getByRole("button", { name: "Wrap", exact: true }).click();
		const grayscale = await appWindow.addStyleTag({
			content: "#details-panel { filter: grayscale(1); }",
		});
		await shoot(appWindow, "replacement-grayscale", "#details-panel");
		await grayscale.evaluate((element) => element.parentNode?.removeChild(element));
	}
});
