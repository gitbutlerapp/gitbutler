import type { ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("shows PR titles and labels on workspace branches without hover shifts", async ({
	appWindow,
	electronApp,
}) => {
	const review: ForgeReview = {
		number: 1,
		title: "Improve keyboard navigation in large repositories",
		sourceBranch: "C",
		targetBranch: "master",
		htmlUrl: "https://example.com/repo/pull/1",
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
	await electronApp.evaluate(({ ipcMain }, review) => {
		ipcMain.removeHandler("forgeInfo");
		ipcMain.handle(
			"forgeInfo",
			() =>
				({
					name: "github",
					baseUrl: "https://example.com/repo",
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
				}) satisfies ForgeInfo,
		);
		ipcMain.removeHandler("listReviews");
		ipcMain.handle("listReviews", () => [review]);
	}, review);
	await appWindow.reload();
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
