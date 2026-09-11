import { expect, test } from "../test.ts";

test.describe("hidden upstream toolbar", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("hides the docked fetch button while the upstream row is visible", async ({ appWindow }) => {
		const sidebar = appWindow.locator("#sidebar-panel");
		const docked = sidebar.locator('[class*="docked"]');
		await expect(docked).toBeHidden();
		await expect(
			docked.getByRole("button", { name: "Fetch", exact: true, includeHidden: true }),
		).toBeHidden();
		await expect(sidebar.getByRole("button", { name: "Fetch", exact: true })).toHaveCount(1);
	});
});

test.describe("docked upstream toolbar", () => {
	test.use({ scenario: "project-with-many-independent-stacks.sh" });

	test("shows fetch with the docked row and hides both after scrolling to upstream", async ({
		appWindow,
	}) => {
		const sidebar = appWindow.locator("#sidebar-panel");
		const docked = sidebar.locator('[class*="docked"]');
		const fetch = docked.getByRole("button", { name: "Fetch", exact: true, includeHidden: true });
		await expect(fetch).toBeInViewport();
		await appWindow
			.getByRole("treeitem", { name: /^stack-\d+$/ })
			.first()
			.evaluate((row) => {
				let scroller = row.parentElement;
				while (scroller && getComputedStyle(scroller).overflowY !== "auto")
					scroller = scroller.parentElement;
				if (!scroller) throw new Error("No workspace scroller");
				scroller.scrollTop = scroller.scrollHeight;
			});
		await expect(docked).toBeHidden();
		await expect(fetch).toBeHidden();
		await expect(sidebar.getByRole("button", { name: "Fetch", exact: true })).toHaveCount(1);
	});
});
