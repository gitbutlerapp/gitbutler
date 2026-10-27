import { expect, test } from "../test.ts";

const lanes = [
	{
		lane: "stack",
		scenario: "project-in-single-branch-three-branch-stack.sh",
		branch: "B",
		commit: "B: first commit",
	},
	{
		lane: "worktree",
		scenario: "project-in-single-branch-with-worktree.sh",
		branch: "W",
		commit: "W: first commit",
	},
];

for (const { lane, scenario, branch, commit } of lanes) {
	test.describe(`a ${lane} branch`, () => {
		test.use({ scenario });

		test("folding hands its selected commit's place to the branch", async ({ app }) => {
			await app.selectCommit(commit);

			await app.branch(branch).getByRole("button", { name: "Fold commits" }).click();

			await expect(app.commit(commit)).toBeHidden();
			await expect(app.branch(branch)).toBeSelected();
		});

		test("the fold key folds the selected branch", async ({ app }) => {
			await app.selectBranch(branch);

			await app.press("z");

			await expect(
				app.branch(branch).getByRole("button", { name: "Unfold commits" }),
			).toBeVisible();
			await expect(app.commit(commit)).toBeHidden();
		});
	});
}
