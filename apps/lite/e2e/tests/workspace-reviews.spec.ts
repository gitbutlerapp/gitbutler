import type { ForgeInfo, ForgeReview, RefInfo } from "@gitbutler/but-sdk";
import type { Page } from "@playwright/test";
import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import { expect, test as base } from "../test.ts";

const test = base.extend<{ reviewState: "none" | "open" | "merged" | "closed" }>({
	reviewState: ["open", { option: true }],
});
test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

const review: ForgeReview = {
	number: 1,
	title: "Improve keyboard navigation in large repositories",
	sourceBranch: "C",
	targetBranch: "master",
	htmlUrl: "https://github.com/example/repo/pull/1",
	body: null,
	author: null,
	labels: [
		{ name: "accessibility", color: "0e8a16", description: "Keyboard and screen reader support" },
		{ name: "@gitbutler/lite", color: "5319e7", description: null },
	],
	draft: false,
	sha: "0000000000000000000000000000000000000001",
	integrationCommitShas: [],
	createdAt: "2026-08-01T00:00:00Z",
	modifiedAt: "2026-08-02T00:00:00Z",
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [],
	autoMergeEnabled: false,
	unitSymbol: "#",
	lastSyncAt: "2026-08-02T00:00:00Z",
};

test.beforeEach(async ({ appWindow, electronApp, reviewState }) => {
	// The open-PR list omits merged and closed PRs. Their recorded branch metadata
	// lets the app retrieve them through getReview instead.
	let headInfo: RefInfo | undefined;
	if (reviewState === "merged" || reviewState === "closed") {
		headInfo = await appWindow.evaluate(async () => {
			const lite = (window as unknown as { lite: LiteElectronApi }).lite;
			const projectId = location.pathname.split("/")[2];
			if (projectId === undefined) throw new Error("No project in the URL");
			return lite.headInfo(projectId);
		});
		const branch = headInfo.stacks
			.flatMap((stack) => stack.segments)
			.find((segment) => segment.refName?.displayName === review.sourceBranch);
		if (!branch) throw new Error("The seeded branch is missing");
		branch.metadata = {
			refInfo: { createdAt: null, updatedAt: null },
			review: { pullRequest: review.number, reviewId: null },
		};
	}
	await electronApp.evaluate(
		({ ipcMain }, { headInfo, review, reviewState }) => {
			const responses = {
				...(headInfo ? { headInfo } : {}),
				forgeInfo: {
					name: "github",
					baseUrl: "https://github.com/example/repo",
					commitUrlPath: "/commit/",
					prUrlPath: "/pull/",
					unit: { symbol: "#", name: "pull request", abbr: "PR" },
					posthogLabel: "",
					capabilities: {
						prService: true,
						checks: false,
						repoInfo: false,
						listService: false,
						reviewComments: false,
						reviewManagement: false,
					},
				} satisfies ForgeInfo,
				listReviews: reviewState === "open" ? [review] : [],
				getReview: {
					...review,
					mergedAt: reviewState === "merged" ? "2026-08-03T00:00:00Z" : null,
					closedAt: reviewState === "open" ? null : "2026-08-03T00:00:00Z",
				},
				listKnownGithubAccounts: [{ type: "patUsername", info: { username: "test" } }],
			};
			for (const [key, value] of Object.entries(responses)) {
				ipcMain.removeHandler(key);
				ipcMain.handle(key, () => value);
			}
		},
		{ headInfo, review, reviewState },
	);
	await appWindow.reload();
});

const selectBranch = (page: Page, name: string) =>
	page.getByRole("treeitem", { name, exact: true }).getByTitle(name, { exact: true }).click();
const branchTabs = (page: Page) => page.getByRole("group", { name: "Branch tab", exact: true });

test("shows PR titles and labels on workspace branches without hover shifts", async ({
	appWindow,
}) => {
	const branch = appWindow.getByRole("treeitem", { name: "C", exact: true });
	await expect(branch.getByText(review.title, { exact: true })).toBeVisible();
	await expect(branch.getByText("accessibility", { exact: true })).toBeVisible();
	await expect(branch.getByText("@gitbutler/lite", { exact: true })).toBeVisible();
	await expect(branch.getByText("PR #1", { exact: true })).toHaveCount(0);
	const name = branch.getByTitle("C", { exact: true });
	const status = branch.getByText("Unpushed branch", { exact: true });
	const push = branch.getByRole("button", {
		name: "Push this and all branches below",
		exact: true,
	});
	const [nameBox, statusBox, pushBox] = await Promise.all([
		name.boundingBox(),
		status.boundingBox(),
		push.boundingBox(),
	]);
	if (!nameBox || !statusBox || !pushBox) throw new Error("Branch push row has no bounds");
	const centers = [nameBox, statusBox, pushBox].map((box) => box.y + box.height / 2);
	expect(Math.max(...centers) - Math.min(...centers)).toBeLessThanOrEqual(1);
	expect(nameBox.x + nameBox.width).toBeLessThan(statusBox.x);
	expect(statusBox.x + statusBox.width).toBeLessThan(pushBox.x);
	const labelBounds = await branch
		.getByText("@gitbutler/lite", { exact: true })
		.evaluate((element) => {
			const text = document.createRange();
			text.selectNodeContents(element);
			return {
				glyphBottom: text.getBoundingClientRect().bottom,
				clipBottom: element.getBoundingClientRect().bottom,
			};
		});
	expect(labelBounds.glyphBottom).toBeLessThanOrEqual(labelBounds.clipBottom);

	await expect(branch).toHaveAccessibleDescription(
		/Improve keyboard navigation.*accessibility.*@gitbutler\/lite/,
	);
	await expect(
		branch.getByTitle("C", { exact: true }).locator("..").locator("[data-icon]"),
	).toBeVisible();
	await appWindow.mouse.move(0, 0);
	const headline = branch.getByText(review.title, { exact: true }).locator("..");
	const before = await headline.boundingBox();
	await headline.hover();
	await expect(branch.getByRole("button", { name: "Branch menu", exact: true })).toBeVisible();
	expect(await headline.boundingBox()).toEqual(before);
	await branch.getByTitle("C", { exact: true }).click();
	await appWindow.keyboard.press("F2");
	const editor = branch.getByRole("textbox", { name: "Branch name" });
	await expect(editor).toHaveValue("C");
	await expect(branch).not.toHaveAttribute("aria-describedby");
	await editor.press("Escape");
	await expect(branch.getByText("accessibility", { exact: true })).toBeVisible();
});

for (const { reviewState, defaultTab, showCreateButton } of [
	{ reviewState: "none", defaultTab: "Diff", showCreateButton: true },
	{ reviewState: "open", defaultTab: "Pull Request", showCreateButton: false },
	{ reviewState: "merged", defaultTab: "Pull Request", showCreateButton: false },
	{ reviewState: "closed", defaultTab: "Diff", showCreateButton: true },
] as const) {
	test.describe(`applied branch, PR state: ${reviewState}`, () => {
		test.use({ reviewState });
		test(`defaults to ${defaultTab} and remembers a tab change`, async ({ appWindow }) => {
			await selectBranch(appWindow, "C");
			const tabs = branchTabs(appWindow);
			const createPullRequest = appWindow.getByRole("button", {
				name: "Create pull request",
				exact: true,
			});
			await expect(createPullRequest).toBeVisible({ visible: showCreateButton });
			await expect(tabs.getByRole("button", { name: defaultTab, pressed: true })).toBeVisible();
			await expect(appWindow.getByPlaceholder("PR title")).toBeHidden();

			const chosenTab = defaultTab === "Diff" ? "Pull Request" : "Diff";
			await tabs.getByRole("button", { name: chosenTab, exact: true }).click();
			await selectBranch(appWindow, "B");
			await selectBranch(appWindow, "C");
			await expect(tabs.getByRole("button", { name: chosenTab, pressed: true })).toBeVisible();
		});
	});
}

test.describe("creating a PR", () => {
	test.use({ reviewState: "none" });
	test("opens the form from the branch header", async ({ appWindow }) => {
		await selectBranch(appWindow, "C");
		await expect(appWindow.getByPlaceholder("PR title")).toBeHidden();
		await appWindow.getByRole("button", { name: "Create pull request", exact: true }).click();
		await expect(appWindow.getByPlaceholder("PR title")).toBeVisible();
	});
});
