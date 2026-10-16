import { assertBranch } from "../src/branch.ts";
import { expect } from "../src/expect.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, stack, waitForTestId } from "../src/util.ts";
import { execFileSync } from "node:child_process";
import type { Page } from "@playwright/test";

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", ["-C", pathToRepo, ...args], { encoding: "utf8" }).trim();
}

async function createDependentBranchFromMainModal(
	page: Page,
	branchName: string,
	stackName: string,
) {
	await clickByTestId(page, "chrome-header-create-branch-button");
	const modal = await waitForTestId(page, "create-new-branch-modal");
	await modal.locator("#new-branch-name-input").fill(branchName);
	await modal.locator('label[for="new-dependent"]').click();
	await modal.getByPlaceholder("Select a stack...").click();
	await page.getByRole("listbox").getByText(stackName, { exact: true }).click();
	await modal.getByTestId("confirm-submit").click();
	await expect(modal).not.toBeVisible();
}

test("main branch modal creates a dependent branch in the selected stack", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);
	await expect(stack(page)).toHaveCount(2);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	const firstTip = git(localClone, ["rev-parse", "branch1"]);
	const secondTip = git(localClone, ["rev-parse", "branch2"]);

	await createDependentBranchFromMainModal(page, "selected-stack-top", "branch2");

	await expect(stack(page)).toHaveCount(2);
	await expect(stack(page, "branch2").getByTestId("branch-header")).toHaveCount(2);
	await expect(stack(page, "branch2").getByTestId("branch-header")).toContainText([
		"selected-stack-top",
		"branch2",
	]);
	await expect(stack(page, "branch1").getByTestId("branch-header")).toHaveCount(1);
	expect(git(localClone, ["rev-parse", "selected-stack-top"])).toBe(secondTip);
	expect(git(localClone, ["rev-parse", "branch1"])).toBe(firstTip);
	expect(git(localClone, ["rev-parse", "branch2"])).toBe(secondTip);
	await assertBranch("gitbutler/workspace", localClone);
});

test("main branch modal creates above an empty top branch without losing it", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);
	await expect(stack(page)).toHaveCount(2);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	const firstTip = git(localClone, ["rev-parse", "branch1"]);
	const secondTip = git(localClone, ["rev-parse", "branch2"]);

	await createDependentBranchFromMainModal(page, "empty-lower", "branch2");
	await expect(stack(page, "branch2").getByTestId("branch-header")).toContainText([
		"empty-lower",
		"branch2",
	]);
	await createDependentBranchFromMainModal(page, "empty-upper", "empty-lower");

	await expect(stack(page)).toHaveCount(2);
	await expect(stack(page, "branch2").getByTestId("branch-header")).toHaveCount(3);
	await expect(stack(page, "branch2").getByTestId("branch-header")).toContainText([
		"empty-upper",
		"empty-lower",
		"branch2",
	]);
	await expect(stack(page, "branch1").getByTestId("branch-header")).toHaveCount(1);
	expect(git(localClone, ["rev-parse", "empty-upper"])).toBe(secondTip);
	expect(git(localClone, ["rev-parse", "empty-lower"])).toBe(secondTip);
	expect(git(localClone, ["rev-parse", "branch1"])).toBe(firstTip);
	expect(git(localClone, ["rev-parse", "branch2"])).toBe(secondTip);
	await assertBranch("gitbutler/workspace", localClone);
});
