import {
	applyBranchFromBranchesView,
	branchHeader,
	createDependentBranch,
	expectCurrentBranchChip,
	openSingleBranchWorkspace,
	setupSingleBranchProject,
	SINGLE_BRANCH_NAME,
} from "./helpers.ts";
import {
	assertBranch,
	assertCommitSubjects,
	assertSymbolicHead,
	branchTip,
	createNewBranch,
} from "../../src/branch.ts";
import { updateCommitMessage } from "../../src/commit.ts";
import { writeToFile } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import {
	clickByTestId,
	commitRow,
	dragAndDropByLocator,
	getByTestId,
	stack,
} from "../../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import type { GitButler } from "../../src/setup.ts";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("keeps managed branch order after checking out a middle branch", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openSingleBranchWorkspace(page);
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await createNewBranch(page, "A");
	await expect(stack(page)).toHaveCount(1);
	await createNewBranch(page, "B");
	await expect(stack(page)).toHaveCount(2);

	await createDependentBranch(page, "D", "A");
	await createDependentBranch(page, "C", "A");
	await expectBranchHeaderOrder(page, ["C", "D", "A"], "A");

	await dragAndDropByLocator(page, branchHeader(page, "A"), branchHeader(page, "D"), {
		force: true,
		position: { x: 120, y: 0 },
	});
	await expectBranchHeaderOrder(page, ["C", "A", "D"], "A");

	execFileSync("git", ["checkout", "A"], { cwd: localClone });

	await assertSymbolicHead("A", localClone);
	await expect(stack(page)).toHaveCount(1);
	await expectBranchHeaderOrder(page, ["A", "D"]);
});

test("keeps the ad-hoc stack when applying an independent empty branch", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openSingleBranchWorkspace(page);
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await createNewBranch(page, "A");
	await expect(stack(page)).toHaveCount(1);
	await createDependentBranch(page, "B", "A");
	await expectBranchHeaderOrder(page, ["B", "A"], "A");
	await createDependentBranch(page, "C", "A");
	await expectBranchHeaderOrder(page, ["C", "B", "A"], "A");
	await createNewBranch(page, "D");
	await expect(stack(page)).toHaveCount(2);

	execFileSync("git", ["checkout", "C"], { cwd: localClone });
	await assertSymbolicHead("C", localClone);
	await expect(stack(page)).toHaveCount(1);
	await expectBranchHeaderOrder(page, ["C", "B", "A"]);

	await applyBranchFromBranchesView(page, "D");
	await assertSymbolicHead("gitbutler/workspace", localClone);
	await expect(stack(page)).toHaveCount(2);
	await expectBranchHeaderOrder(page, ["C", "B", "A"], "A");
	await expectBranchHeaderOrder(page, ["D"], "D");
});

test("can reorder empty branches by dragging within the single-branch stack", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	// Build a stack of empty branches. Each `createDependentBranch` is created above the current tip
	// and checked out, so `empty-top` ends up as the checked-out entrypoint with two empty branches
	// (`empty-mid`, `empty-low`) below it, sitting on the commit-owning `single-branch-fixture` base.
	await createDependentBranch(page, "empty-low");
	await createDependentBranch(page, "empty-mid");
	await createDependentBranch(page, "empty-top");

	await expect(getByTestId(page, "branch-card")).toHaveCount(4);
	await expectCurrentBranchChip(page, "empty-top");
	await expectBranchHeaderOrder(page, ["empty-top", "empty-mid", "empty-low", SINGLE_BRANCH_NAME]);

	// Drag `empty-low` onto the insertion dropzone above `empty-mid` to put it on top of `empty-mid`.
	// Both branches are empty and below the entrypoint, so the whole stack stays projected and the
	// move is a pure `branch_order` metadata reorder.
	//
	await dragAndDropByLocator(
		page,
		branchHeader(page, "empty-low"),
		branchHeader(page, "empty-mid"),
		{ force: true, position: { x: 120, y: 0 } },
	);

	// `empty-low` now sits above `empty-mid`; the entrypoint and base are unchanged.
	await expectBranchHeaderOrder(page, ["empty-top", "empty-low", "empty-mid", SINGLE_BRANCH_NAME]);
	await expectCurrentBranchChip(page, "empty-top");
	await assertBranch("empty-top", localClone);
});

test("moving an empty branch above the checked-out branch checks out the new tip", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	// `empty-b` is created last, so it's the checked-out tip, with `empty-a` below it.
	await createDependentBranch(page, "empty-a");
	await createDependentBranch(page, "empty-b");

	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectBranchHeaderOrder(page, ["empty-b", "empty-a", SINGLE_BRANCH_NAME]);
	await expectCurrentBranchChip(page, "empty-b");
	await assertBranch("empty-b", localClone);

	// Drag `empty-a` onto the dropzone above the checked-out tip `empty-b` (the top dropzone renders
	// while the drag is active). This places `empty-a` above the entrypoint, so it becomes the new
	// tip. Because the move can't leave the new tip above `HEAD` unprojected, the backend reports it
	// as the new tip and the app checks it out.
	await dragAndDropByLocator(page, branchHeader(page, "empty-a"), branchHeader(page, "empty-b"), {
		force: true,
		position: { x: 120, y: -10 },
	});

	// `empty-a` is now the tip and is checked out; the whole stack stays projected.
	await expectBranchHeaderOrder(page, ["empty-a", "empty-b", SINGLE_BRANCH_NAME]);
	await expectCurrentBranchChip(page, "empty-a");
	await assertBranch("empty-a", localClone);
});

test("keeps empty dependent branches when moving their commit-owning branch to the top", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	const commitBranch = "commit-branch";
	const emptyLow = "empty-low";
	const emptyTop = "empty-top";

	await createDependentBranch(page, commitBranch);
	const fileName = "commit_branch.txt";
	writeToFile(gitbutler.pathInWorkdir("local-clone", fileName), "commit branch content\n");
	await expect(getByTestId(page, "file-list-item").filter({ hasText: fileName })).toBeVisible();
	await clickByTestId(page, "start-commit-button");
	const commitTitle = "commit branch: add commit";
	await updateCommitMessage(page, commitTitle, "");
	await clickByTestId(page, "commit-drawer-action-button");
	await expect(commitRow(page, commitTitle)).toBeVisible();

	await createDependentBranch(page, emptyLow);
	await createDependentBranch(page, emptyTop);
	await expectBranchHeaderOrder(page, [emptyTop, emptyLow, commitBranch, SINGLE_BRANCH_NAME]);
	const commitTip = branchTip(commitBranch, localClone);
	expect(branchTip(emptyTop, localClone)).toBe(commitTip);
	expect(branchTip(emptyLow, localClone)).toBe(commitTip);

	await dragBranchToInsertionDropzone(page, commitBranch, 0);

	await expectBranchHeaderOrder(page, [commitBranch, emptyTop, emptyLow, SINGLE_BRANCH_NAME]);
	await expectCurrentBranchChip(page, commitBranch);
	await assertBranch(commitBranch, localClone);
	expect(branchTip(commitBranch, localClone)).toBe(commitTip);
	expect(branchTip(emptyTop, localClone)).toBe(branchTip(SINGLE_BRANCH_NAME, localClone));
	expect(branchTip(emptyLow, localClone)).toBe(branchTip(SINGLE_BRANCH_NAME, localClone));
});

test("can move a middle non-empty branch above the checked-out branch", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupThreeBranchStackProject(gitbutler, page);

	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectCurrentBranchChip(page, "C");
	await expectBranchHeaderOrder(page, ["C", "B", "A"]);

	await dragBranchToInsertionDropzone(page, "B", 0);

	await expectBranchHeaderOrder(page, ["B", "C", "A"]);
	await expectCurrentBranchChip(page, "B");
	await assertBranch("B", localClone);
	await assertCommitSubjects(["B: first commit", "C: first commit", "A: first commit"], localClone);
});

test("can move a bottom non-empty branch above the checked-out branch", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupThreeBranchStackProject(gitbutler, page);

	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectCurrentBranchChip(page, "C");
	await expectBranchHeaderOrder(page, ["C", "B", "A"]);

	await dragBranchToInsertionDropzone(page, "A", 0);

	await expectBranchHeaderOrder(page, ["A", "C", "B"]);
	await expectCurrentBranchChip(page, "A");
	await assertBranch("A", localClone);
	await assertCommitSubjects(["A: first commit", "C: first commit", "B: first commit"], localClone);
});

test("can move the checked-out top branch down within its stack", async ({ page, gitbutler }) => {
	const localClone = await setupThreeBranchStackProject(gitbutler, page);

	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectCurrentBranchChip(page, "C");
	await expectBranchHeaderOrder(page, ["C", "B", "A"]);

	await dragBranchToInsertionDropzone(page, "C", 1);

	await expectBranchHeaderOrder(page, ["B", "C", "A"]);
	await expectCurrentBranchChip(page, "B");
	await assertBranch("B", localClone);
	await assertCommitSubjects(["B: first commit", "C: first commit", "A: first commit"], localClone);
});

test("keeps an empty top branch empty when moving it below a commit-owning branch", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupThreeBranchStackProject(gitbutler, page, "C", true);
	const bTipBefore = branchTip("B", localClone);
	const aTipBefore = branchTip("A", localClone);

	await expect(getByTestId(page, "branch-card")).toHaveCount(3);
	await expectCurrentBranchChip(page, "C");
	await expectBranchHeaderOrder(page, ["C", "B", "A"]);
	expect(branchTip("C", localClone)).toBe(bTipBefore);

	const bCard = getByTestId(page, "branch-card").filter({ has: branchHeader(page, "B") });
	const cCard = getByTestId(page, "branch-card").filter({ has: branchHeader(page, "C") });
	await expect(
		bCard.getByTestId("commit-row").filter({ hasText: "B: first commit" }),
	).toBeVisible();
	await expect(cCard.getByTestId("commit-row")).toHaveCount(0);

	// Moving C below B drops it immediately above A. C must move to A's tip so B keeps its commit.
	await dragBranchToInsertionDropzone(page, "C", 1);

	await expectBranchHeaderOrder(page, ["B", "C", "A"]);
	await expectCurrentBranchChip(page, "B");
	await assertBranch("B", localClone);
	expect(branchTip("B", localClone)).toBe(bTipBefore);
	expect(branchTip("C", localClone)).toBe(aTipBefore);
	await expect(
		bCard.getByTestId("commit-row").filter({ hasText: "B: first commit" }),
	).toBeVisible();
	await expect(cCard.getByTestId("commit-row")).toHaveCount(0);
	await assertCommitSubjects(["B: first commit", "A: first commit"], localClone);
});

test("can move a bottom non-empty branch above the checked-out middle branch", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupThreeBranchStackProject(gitbutler, page, "B");
	const cTipBefore = branchTip("C", localClone);

	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expectCurrentBranchChip(page, "B");
	await expectBranchHeaderOrder(page, ["B", "A"]);

	await dragBranchToInsertionDropzone(page, "A", 0);

	await expectBranchHeaderOrder(page, ["A", "B"]);
	await expectCurrentBranchChip(page, "A");
	await assertBranch("A", localClone);
	expect(branchTip("C", localClone)).toBe(cTipBefore);
	await assertCommitSubjects(["A: first commit", "B: first commit"], localClone);
});

async function setupThreeBranchStackProject(
	gitbutler: GitButler,
	page: Page,
	headBranch = "C",
	emptyTopBranch = false,
): Promise<string> {
	await gitbutler.runScript("project-in-single-branch-three-branch-stack.sh", [
		headBranch,
		emptyTopBranch.toString(),
	]);
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);
	return localClone;
}

async function dragBranchToInsertionDropzone(
	page: Page,
	branchName: string,
	dropzoneIndex: number,
): Promise<void> {
	const source = branchHeader(page, branchName);
	await source.hover();
	const box = await source.boundingBox();
	if (!box) throw new Error(`Branch header ${branchName} has no bounding box`);

	await page.mouse.down();
	await page.mouse.move(box.x + 16, box.y + 16);
	await page.evaluate(
		async () => await new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
	);

	const target = page.getByTestId("BranchListInsertionDropzone").nth(dropzoneIndex);
	await target.hover({ force: true, position: { x: 120, y: 6 } });
	await page.evaluate(
		async () => await new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
	);
	await page.mouse.up();
}

async function expectBranchHeaderOrder(
	page: Page,
	expectedBranchNames: string[],
	stackBranch?: string,
): Promise<void> {
	const root = stackBranch ? stack(page).filter({ has: branchHeader(page, stackBranch) }) : page;
	await expect
		.poll(
			async () =>
				await root
					.locator('[data-testid="branch-header"]')
					.evaluateAll((headers) =>
						headers.map((header) => header.getAttribute("data-testid-branch-header")),
					),
			{
				message: `Expected branch header order ${JSON.stringify(expectedBranchNames)}`,
				intervals: [100, 200, 500, 1000],
			},
		)
		.toEqual(expectedBranchNames);
}
