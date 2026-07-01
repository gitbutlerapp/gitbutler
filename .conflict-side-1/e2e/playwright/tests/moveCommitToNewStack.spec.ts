import { applyUpstream, openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { dragAndDropByLocator, MOD_KEY, stack, waitForTestId } from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

/**
 * Apply branch2 (modifies b_file) and branch3 (modifies c_file) — two
 * stacks independent of master's a_file, so moves don't conflict.
 */
async function applyTwoIndependentStacks(gitbutler: GitButler, page: Page) {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch2", "branch3");
	await openWorkspace(page);
	await expect(stack(page)).toHaveCount(2);
}

test("move a commit to the new stack dropzone to create a new stack", async ({
	page,
	gitbutler,
}) => {
	test.setTimeout(120_000);
	await applyTwoIndependentStacks(gitbutler, page);

	const stack2 = stack(page, "branch2");
	await expect(stack2.getByTestId("commit-row")).toHaveCount(2);

	const commitToDrag = stack2.getByTestId("commit-row").filter({
		hasText: "branch2: first commit",
	});
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, commitToDrag, stackDropzone, {
		force: true,
		position: { x: 10, y: 10 },
	});

	await expect(stack(page)).toHaveCount(3, { timeout: 15_000 });
	await expect(stack2.getByTestId("commit-row")).toHaveCount(1, { timeout: 15_000 });
	await expect(
		stack2.getByTestId("commit-row").filter({ hasText: "branch2: first commit" }),
	).toHaveCount(0);
});

test("move multiple selected commits to the new stack dropzone", async ({ page, gitbutler }) => {
	test.setTimeout(120_000);
	await applyTwoIndependentStacks(gitbutler, page);

	const stack3 = stack(page, "branch3");
	const commits = stack3.getByTestId("commit-row");
	await expect(commits).toHaveCount(2);

	const firstCommit = commits.filter({ hasText: "branch3: first commit" });
	const secondCommit = commits.filter({ hasText: "branch3: second commit" });

	await firstCommit.click();
	await secondCommit.click({ modifiers: [MOD_KEY] });
	await expect(firstCommit).toHaveClass(/\bselected\b/);
	await expect(secondCommit).toHaveClass(/\bselected\b/);

	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, firstCommit, stackDropzone, {
		force: true,
		position: { x: 10, y: 10 },
	});

	await expect(stack(page)).toHaveCount(3, { timeout: 15_000 });
	// Original branch3 stack should be empty (both commits moved out).
	await expect(stack3.getByTestId("commit-row")).toHaveCount(0, { timeout: 15_000 });
});
