import { expect, test } from "../test.ts";
import { openReadabilityReview } from "../pr-readability-fixture.ts";

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
