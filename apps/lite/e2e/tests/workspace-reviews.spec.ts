import type { Locator } from "@playwright/test";
import { aReview } from "../forge.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

const review = aReview({
	sourceBranch: "C",
	title: "Improve keyboard navigation in large repositories",
	labels: [
		{ name: "accessibility", color: "0e8a16", description: "Keyboard and screen reader support" },
		{ name: "@gitbutler/lite", color: "5319e7", description: null },
	],
});
const closedAt = "2026-08-03T00:00:00Z";

test.describe("a branch with an open pull request", () => {
	test.use({ forge: { reviews: [review] } });

	test("shows the PR's title and labels in place of its number", async ({ app }) => {
		const branch = app.branch("C");

		await expect(branch).toShow(review.title);
		await expect(branch).toShow("accessibility");
		await expect(branch).toShow("@gitbutler/lite");
		await expect(branch).not.toShow("PR #1");
		await expect(branch).toHaveAccessibleDescription(
			/Improve keyboard navigation.*accessibility.*@gitbutler\/lite/,
		);
	});

	test("shows its labels unclipped", async ({ app }) => {
		await expect(
			app.branch("C").getByText("@gitbutler/lite", { exact: true }),
		).toShowItsTextUnclipped();
	});

	test("keeps the name, push status and push button on one line", async ({ app }) => {
		const branch = app.branch("C");

		await expectInOrderOnOneLine(
			app.branchName("C"),
			branch.getByText("Unpushed branch", { exact: true }),
			branch.getByRole("button", { name: "Push this and all branches below", exact: true }),
		);
	});

	test("hovering the PR headline moves nothing", async ({ app }) => {
		const headline = app.branch("C").getByText(review.title, { exact: true }).locator("..");

		await expect(headline).toHoldStillOnHover({ revealing: app.branchMenu("C") });
	});

	test("renaming drops the PR description until the edit ends", async ({ app }) => {
		const editor = await app.editBranchName("C");

		await expect(editor).toHaveValue("C");
		await expect(app.branch("C")).not.toHaveAttribute("aria-describedby");
		await editor.press("Escape");
		await expect(app.branch("C")).toShow("accessibility");
	});
});

for (const { state, forge, defaultTab } of [
	{ state: "no PR", forge: { reviews: [] }, defaultTab: "Diff" },
	{ state: "an open PR", forge: { reviews: [review] }, defaultTab: "PR #1" },
	{
		state: "a merged PR",
		forge: { reviews: [{ ...review, mergedAt: closedAt, closedAt }] },
		defaultTab: "Pull Request",
	},
	{ state: "a closed PR", forge: { reviews: [{ ...review, closedAt }] }, defaultTab: "Diff" },
]) {
	test.describe(`an applied branch with ${state}`, () => {
		test.use({ forge });

		test(`opens on ${defaultTab} and remembers a tab change`, async ({ app }) => {
			await app.selectBranch("C");
			await expect(app.branchTab(defaultTab)).toBeSelected();
			await expect(app.pullRequestForm).toBeHidden();

			const chosenTab = defaultTab === "Diff" ? "Pull Request" : "Diff";
			await app.branchTab(chosenTab).click();
			await app.selectBranch("B");
			await app.selectBranch("C");

			await expect(app.branchTab(chosenTab)).toBeSelected();
		});
	});
}

test.describe("a branch without a PR", () => {
	test.use({ forge: { reviews: [] } });

	test("opens the PR form from the branch header", async ({ app }) => {
		await app.selectBranch("C");
		await app.branchTab("Pull Request").click();

		await expect(app.pullRequestForm).toBeVisible();
	});
});

/** Each element sits left of the next, all centred on the same line. */
const expectInOrderOnOneLine = async (...elements: Array<Locator>): Promise<void> => {
	const boxes = await Promise.all(
		elements.map(async (element) => {
			const box = await element.boundingBox();
			if (box === null) throw new Error(`${String(element)} has no bounding box`);
			return box;
		}),
	);
	const centers = boxes.map((box) => box.y + box.height / 2);
	expect(Math.max(...centers) - Math.min(...centers)).toBeLessThanOrEqual(1);
	boxes.reduce((left, right) => {
		expect(left.x + left.width).toBeLessThan(right.x);
		return right;
	});
};
