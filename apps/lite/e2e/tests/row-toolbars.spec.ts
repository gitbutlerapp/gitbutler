import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

const fetchButtons = (app: App) => ({
	docked: app
		.getByRole("group", { name: "Docked upstream", includeHidden: true })
		.getByRole("button", { name: "Fetch", exact: true, includeHidden: true }),
	visible: app.page.locator("#sidebar-panel").getByRole("button", { name: "Fetch", exact: true }),
});

test.describe("hidden upstream toolbar", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("hides the docked fetch button while the upstream row is visible", async ({ app }) => {
		await expect(fetchButtons(app).docked).toBeHidden();
		await expect(fetchButtons(app).visible).toHaveCount(1);
	});
});

test.describe("docked upstream toolbar", () => {
	test.use({ scenario: "project-with-many-independent-stacks.sh" });

	test("shows fetch while docked", async ({ app }) => {
		await expect(fetchButtons(app).docked).toBeInViewport();
	});

	test("hides it after scrolling to the upstream row", async ({ app }) => {
		await app.listScroll(app.getByRole("treeitem", { name: /^stack-\d+$/ }).first(), "end");

		await expect(fetchButtons(app).docked).toBeHidden();
		await expect(fetchButtons(app).visible).toHaveCount(1);
	});
});
