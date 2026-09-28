import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("folding a branch hands its selected commit's place to the branch", async ({ appWindow }) => {
	const branch = appWindow.getByRole("treeitem", { name: "B", exact: true });
	const commit = appWindow.getByRole("treeitem", { name: "B: first commit", exact: true });
	await commit.click();
	await expect(commit).toHaveAttribute("aria-selected", "true");

	await branch.getByRole("button", { name: "Fold commits" }).click();

	await expect(commit).toBeHidden();
	await expect(branch).toHaveAttribute("aria-selected", "true");
});
