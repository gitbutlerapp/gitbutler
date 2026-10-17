import type { ListedStack, ListedBranch } from "@gitbutler/but-sdk";
import type { Locator } from "@playwright/test";
import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

/**
 * Unfold every branch in the list. It virtualises, so unfolding what is mounted
 * stages more rows to unfold: walk down it until the last of them shows its commits.
 */
const unfoldEveryBranch = async (app: App, row: Locator): Promise<void> => {
	await row.hover();
	for (let pass = 0; pass < 10; pass++) {
		// One round trip for every mounted toggle.
		await row.evaluate((row) => {
			const toggles = row
				.closest('[role="tree"]')
				?.querySelectorAll<HTMLElement>('button[aria-label="Unfold commits"]');
			for (const toggle of toggles ?? []) toggle.click();
		});
		if (await app.isScrolledToEnd(row)) break;
		await app.page.mouse.wheel(0, 600);
	}
	await app.listScroll(row, 0);
};

test.describe("branches list", () => {
	test.use({ scenario: "project-with-many-unapplied-branches.sh" });

	// Each unfolded branch runs a commit virtualizer on the borrowed list
	// scroller, so with the selected branch scrolled away from, they used to
	// stamp the offset they last remembered back over the scroll the user made.
	test("scrolls to the last branch while the first one is selected", async ({ app }) => {
		// Seeding a project this size and unfolding it costs more than the default budget on CI.
		test.slow();
		await app.goTo("Branches");
		const branches = app.getByRole("treeitem", { name: /^branch-\d+$/ });
		await unfoldEveryBranch(app, branches.first());
		const first = await branches.first().getAttribute("aria-label");
		if (first === null) throw new Error("Branch row has no name");
		await app.selectBranch(first);
		// The branches have to outgrow their panel for any of this to mean anything.
		expect((await app.listScroll(app.branch(first))).end).toBeGreaterThan(0);

		await app.wheelThroughList(app.branchName(first));

		await expect.poll(() => app.isScrolledToEnd(branches.first())).toBe(true);
	});
});

const reviews: Record<string, Partial<ListedBranch>> = {
	branch1: {
		reviewStatus: "open",
		review: {
			number: 42,
			title: "Speed up branch listing in large repositories",
			htmlUrl: "https://example.com/pull/42",
			unitSymbol: "#",
			createdAt: "2026-09-01T12:00:00Z",
			author: { login: "octocat", name: null },
			labels: [
				{ name: "performance", color: "0e8a16", description: "Performance improvements" },
				{ name: "@gitbutler/lite", color: "#ffffff", description: null },
				{ name: "needs review", color: null, description: null },
			],
		},
	},
	branch2: {
		reviewStatus: "draft",
		review: {
			number: 43,
			title: "A draft without optional metadata",
			htmlUrl: "https://example.com/pull/43",
			unitSymbol: "!",
			labels: [],
			author: null,
			createdAt: null,
		},
	},
};

test.describe("recent branch reviews", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test.beforeEach(async ({ app }) => {
		const stacks = await app.page.evaluate((id) => window.lite.branchList(id), app.projectId);
		const withReviews: Array<ListedStack> = stacks.map((stack) => ({
			...stack,
			branches: stack.branches.map((branch) => ({
				...branch,
				...reviews[branch.displayName],
			})),
		}));
		await app.backend.stub({ branchList: withReviews });
		await app.reload();
		await app.goTo("Branches");
	});

	test("shows a branch's review title, labels and author", async ({ app }) => {
		const branch = app.branch("branch1");

		await expect(branch).toShow("Speed up branch listing in large repositories");
		await expect(branch).toShow("performance");
		await expect(branch.getByText(/by octocat/)).toBeVisible();
		await expect(branch).toShow("2 commits");
		await expect(branch).toHaveAccessibleDescription(
			/Speed up branch listing in large repositories.*performance.*octocat/,
		);
	});

	test("marks a draft review without optional metadata", async ({ app }) => {
		await expect(app.branch("branch2")).toShow("Draft");
	});

	test("draws a colourless label as an outlined pill", async ({ app }) => {
		const colorless = app.branch("branch1").getByTitle("needs review", { exact: true });

		await expect(colorless).toHaveCSS("border-top-style", "solid");
		await expect(colorless).not.toHaveCSS("border-top-color", "rgba(0, 0, 0, 0)");
	});

	test("hovering a reviewed branch moves nothing and fits the sidebar", async ({ app }) => {
		const branch = app.branch("branch1");
		// Selected, the menu shows without a hover.
		await app.selectBranch("branch3");

		await expect(
			branch.getByTitle("Speed up branch listing in large repositories", { exact: true }),
		).toHoldStillOnHover({ revealing: app.branchMenu("branch1") });
		await expect(branch.getByText("performance", { exact: true })).toHoldStillOnHover({
			revealing: app.branchMenu("branch1"),
		});
		expect(
			await app.page.locator("#sidebar-panel").evaluate((el) => el.scrollWidth <= el.clientWidth),
		).toBe(true);
	});

	test("opens the review in the browser", async ({ app }) => {
		const opened = await app.backend.record("openInWebBrowser");

		await app.branch("branch1").getByRole("link", { name: "Open #42 in browser" }).click();

		await expect.poll(opened).toEqual([["https://example.com/pull/42"]]);
	});

	test("unfolds a reviewed branch's commits", async ({ app }) => {
		await app.selectBranch("branch1");

		await app.branch("branch1").getByRole("button", { name: "Unfold commits" }).click();

		await expect(
			app.branch("branch1").getByRole("treeitem", { name: "branch1: second commit" }),
		).toBeVisible();
	});

	test("filters by label, author and review number", async ({ app }) => {
		await app.getByRole("button", { name: "Filter branches", exact: true }).click();
		const filter = app.getByRole("textbox", { name: "Filter branches" });

		for (const query of ["performance", "octocat", "#42"]) {
			await filter.fill(query);
			await expect(app.branch("branch1")).toBeVisible();
			await expect(app.branch("branch2")).toHaveCount(0);
		}
		await filter.fill("!43");
		await expect(app.branch("branch2")).toBeVisible();
		await expect(app.branch("branch1")).toHaveCount(0);
	});

	test("says when no branch matches the filter, and Escape clears it", async ({ app }) => {
		await app.getByRole("button", { name: "Filter branches", exact: true }).click();
		const filter = app.getByRole("textbox", { name: "Filter branches" });

		await filter.fill("#999");
		await expect(app).toShow("No branches match");
		await filter.press("Escape");
		await expect(app.branch("branch1")).toBeVisible();
	});
});
