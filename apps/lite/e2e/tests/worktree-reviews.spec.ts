import { aReview } from "../forge.ts";
import { expect, test } from "../test.ts";

test.use({
	scenario: "project-in-single-branch-with-worktree.sh",
	forge: { reviews: [] },
});

test("a worktree branch opens a pull request", async ({ app }) => {
	await app.selectBranch("W");
	await app.branchTab("Pull Request").click();

	await expect(app.pullRequestForm).toBeVisible();
});

test.describe("with an open pull request", () => {
	const review = aReview({ sourceBranch: "W", title: "Teach the worktree to fly" });
	test.use({ forge: { reviews: [review] } });

	test("a worktree branch row shows its title", async ({ app }) => {
		await expect(app.branch("W").getByTitle(review.title)).toBeVisible();
	});
});
