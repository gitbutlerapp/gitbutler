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

for (const { lane, scenario, branch: branchName, commit: commitTitle } of lanes) {
	test.describe(`a ${lane} branch`, () => {
		test.use({ scenario });

		test("folding hands its selected commit's place to the branch", async ({ appWindow }) => {
			const branch = appWindow.getByRole("treeitem", { name: branchName, exact: true });
			const commit = appWindow.getByRole("treeitem", { name: commitTitle, exact: true });
			await commit.click();
			await expect(commit).toHaveAttribute("aria-selected", "true");

			await branch.getByRole("button", { name: "Fold commits" }).click();

			await expect(commit).toBeHidden();
			await expect(branch).toHaveAttribute("aria-selected", "true");
		});
	});
}
