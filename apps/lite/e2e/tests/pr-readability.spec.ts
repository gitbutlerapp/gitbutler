import { expect, test } from "../test.ts";
import { openReadabilityReview, review } from "../pr-readability-fixture.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });
test("makes merge blockers visible and moves verdict headings to bylines", async ({
	appWindow,
	electronApp,
}) => {
	await openReadabilityReview(appWindow, electronApp);
	const merge = appWindow.getByRole("button", { name: "Merge", exact: true });
	await expect(merge).toBeDisabled();
	await expect(
		appWindow.getByText("Changes recommended · 4 reviews pending", { exact: true }),
	).toBeInViewport();
	const readiness = appWindow.getByRole("region", { name: "Merge readiness" });
	await expect(readiness.getByText("Blocked on review", { exact: true })).toBeInViewport();
	await expect(readiness.getByText("Checks passed", { exact: true })).toBeVisible();
	await expect(
		appWindow.getByText("The keyboard flow needs another pass.", { exact: true }),
	).toBeVisible();
	await expect(
		appWindow.getByRole("heading", { name: /Needs a closer look|Changes recommended/ }),
	).toHaveCount(0);
	await expect(
		appWindow.locator('[data-verdict="pop"]').getByText("Needs a closer look", { exact: true }),
	).toBeVisible();
	await electronApp.evaluate(({ ipcMain }) => {
		ipcMain.removeHandler("getReviewMergeStatus");
		ipcMain.handle("getReviewMergeStatus", () => ({
			isMergeable: true,
			mergeableState: "clean",
			commentsCount: 1,
		}));
	});
	await appWindow.reload();
	await expect(merge).toBeEnabled();
	await expect(readiness.getByText("Ready to merge", { exact: true })).toBeVisible();
	await expect(
		appWindow.getByText("Changes recommended · 4 reviews pending", { exact: true }),
	).toHaveCount(0);
});

test("keeps the description readable, fills the rail, and separates audience from density", async ({
	appWindow,
	electronApp,
}) => {
	await openReadabilityReview(appWindow, electronApp);
	const description = appWindow
		.getByRole("heading", { name: review.title, exact: true })
		.locator("..");
	await expect(description.getByRole("button", { name: "Show more", exact: true })).toHaveCount(0);
	const prose = description
		.getByText("This change makes pull request reviews easier to read.", { exact: true })
		.locator("..");
	await expect(prose).toHaveCSS("font-size", "14px");
	await expect(prose).toHaveCSS("line-height", "22.4px");
	const files = appWindow.getByRole("region", { name: "Files changed", exact: true });
	await expect(files.getByTitle("src/file-0.ts", { exact: true })).toBeVisible();
	await expect(files.getByRole("button", { name: "4 more", exact: true })).toBeVisible();
	await files.getByRole("button", { name: "4 more", exact: true }).click();
	await expect(files.getByTitle("src/file-11.ts", { exact: true })).toHaveCount(1);
	const panel = files.locator("..");
	const scroll = appWindow.locator('[class*="prTabScroll"]');
	const panelBox = await panel.boundingBox();
	const scrollBox = await scroll.boundingBox();
	if (!panelBox || !scrollBox) throw new Error("Expected PR panel bounds");
	expect(
		Math.abs(panelBox.y + panelBox.height - (scrollBox.y + scrollBox.height - 24)),
	).toBeLessThan(3);
	await expect(panel.getByRole("heading", { name: "Owners", exact: true })).toBeVisible();
	await expect(panel.getByText("@gitbutler/lite", { exact: true })).toHaveCSS(
		"font-family",
		'"Geist Mono", monospace',
	);
	await expect(panel.getByRole("heading", { name: "Topics", exact: true })).toBeVisible();
	const audience = appWindow.getByRole("group", { name: "Activity audience" });
	await expect(audience.getByRole("button", { name: "All", exact: true })).toHaveAttribute(
		"aria-pressed",
		"true",
	);
	await audience.getByRole("button", { name: "Humans", exact: true }).click();
	await expect(
		appWindow.getByText("The keyboard flow needs another pass.", { exact: true }),
	).toBeVisible();
	await expect(
		appWindow.getByText("Please add coverage for pending reviews.", { exact: true }),
	).toHaveCount(0);
	const compact = appWindow.getByRole("button", { name: "Compact", exact: true });
	await compact.click();
	await expect(compact).toHaveAttribute("aria-pressed", "true");
	await audience.getByRole("button", { name: "Agents", exact: true }).click();
	await expect(
		appWindow.getByText("Please add coverage for pending reviews.", { exact: true }),
	).toBeVisible();
	await expect(
		appWindow.getByText("The keyboard flow needs another pass.", { exact: true }),
	).toHaveCount(0);
	await expect(compact).toHaveAttribute("aria-pressed", "true");
	await electronApp.evaluate(({ ipcMain }, review) => {
		ipcMain.removeHandler("listReviews");
		ipcMain.handle("listReviews", () => [
			{ ...review, body: `${"A paragraph of review context.\n\n".repeat(20)}Final paragraph.` },
		]);
	}, review);
	await appWindow.reload();
	await description.getByRole("button", { name: "Show more", exact: true }).click();
	await expect(description.getByText("Final paragraph.", { exact: true })).toBeVisible();
});
