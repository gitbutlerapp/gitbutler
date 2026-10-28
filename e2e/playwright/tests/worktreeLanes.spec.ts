import { GIT_CONFIG_GLOBAL } from "../src/env.ts";
import { expect } from "../src/expect.ts";
import { writeToFile } from "../src/file.ts";
import {
	forgeReview,
	gitlabForgeInfo,
	mergeStatus,
	mockForge,
	repoInfo,
	storeFakeGitHubEnterprisePat,
} from "../src/forge.ts";
import { getHunkLineSelector } from "../src/hunk.ts";
import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, commitRow, dragAndDropByLocator, fillByTestId } from "../src/util.ts";
import { type Locator, type Page } from "@playwright/test";
import { execFileSync } from "child_process";
import { existsSync, readFileSync } from "fs";

// The stack master <- A <- B <- C, with the linked worktree `worktree-W` on branch W resting on B.
const FIXTURE = "project-in-single-branch-with-worktree.sh";

test.use({
	gitbutlerOptions: {
		config: { onboardingComplete: true },
	},
});

function lane(page: Page, branchName: string): Locator {
	return page.locator(`[data-testid="stack"][data-testid-stack="${branchName}"]`);
}

function git(cwd: string, args: string[]): string {
	// The fixtures' git identity, so commits work where no global one is set, e.g. on CI.
	const env = { ...process.env, GIT_CONFIG_GLOBAL };
	return execFileSync("git", args, { cwd, encoding: "utf8", env }).trim();
}

test("shows a linked worktree as its own lane", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	await expect(worktreeLane.getByText("worktree on B", { exact: true })).toBeVisible();
	await expect(commitRow(page, "W: first commit")).toHaveCount(1);
	await expect(worktreeLane.getByTestId("commit-row")).toHaveText(/W: first commit/);
});

test("commits a worktree's file onto its branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	await expect(worktreeLane).toBeVisible();
	writeToFile(`${worktree}/second.txt`, "second\n");
	await expect(
		worktreeLane.getByTestId("file-list-item").filter({ hasText: "second.txt" }),
	).toBeVisible();

	await worktreeLane.getByTestId("start-commit-button").click();
	await fillByTestId(page, "commit-drawer-title-input", "W: second commit");
	await page.getByTestId("commit-drawer-action-button").click();

	await expect(worktreeLane.getByTestId("commit-row")).toHaveCount(2);
	expect(git(worktree, ["log", "-1", "--format=%s", "W"])).toBe("W: second commit");
	expect(git(worktree, ["status", "--porcelain"]), "the worktree's file was committed").toBe("");
	expect(
		git(localClone, ["status", "--porcelain"]),
		"the main checkout is untouched by a worktree commit",
	).toBe("");
});

test("commits only a worktree's selected files and lines", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	writeToFile(`${worktree}/W.txt`, "W\nfirst\nsecond\n");
	writeToFile(`${worktree}/skip.txt`, "skip\n");
	function file(name: string) {
		return worktreeLane.getByTestId("file-list-item").filter({ hasText: name });
	}
	await expect(file("W.txt")).toBeVisible();
	await expect(file("skip.txt")).toBeVisible();

	await worktreeLane.getByTestId("start-commit-button").click();
	await file("skip.txt").locator('input[type="checkbox"]').click();
	await file("W.txt").click();
	await page
		.locator(getHunkLineSelector("W.txt", 3, "right"))
		.first()
		.click();
	await fillByTestId(page, "commit-drawer-title-input", "W: partial commit");
	await page.getByTestId("commit-drawer-action-button").click();

	await expect
		.poll(() => git(worktree, ["log", "-1", "--format=%s", "W"]))
		.toBe("W: partial commit");
	expect(git(worktree, ["show", "W:W.txt"]), "only the selected line was committed").toBe(
		"W\nfirst",
	);
	expect(git(worktree, ["diff", "--name-only"]), "the unselected line stays").toBe("W.txt");
	expect(git(worktree, ["ls-files", "--others"]), "the unchecked file stays").toBe("skip.txt");
});

test("amends a worktree commit by dragging its file onto it", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	const stackTip = git(localClone, ["rev-parse", "C"]);
	await openWorkspace(page);

	writeToFile(`${worktree}/W.txt`, "W\namended\n");
	const file = lane(page, "W").getByTestId("file-list-item").filter({ hasText: "W.txt" });
	await expect(file).toBeVisible();

	// A stack's commit only takes the main checkout's changes.
	await dragAndDropByLocator(page, file, commitRow(page, "C: first commit"));
	await dragAndDropByLocator(page, file, commitRow(page, "W: first commit"));

	await expect.poll(() => git(worktree, ["show", "W:W.txt"])).toBe("W\namended");
	expect(git(worktree, ["log", "--format=%s", "-1", "W"])).toBe("W: first commit");
	expect(git(worktree, ["status", "--porcelain"]), "the change moved into the commit").toBe("");
	expect(git(localClone, ["rev-parse", "C"]), "the stack's commit is untouched").toBe(stackTip);
});

/** A file of a worktree lane's commit, with the commit's files open. */
async function committedFile(page: Page, commitTitle: string, fileName: string): Promise<Locator> {
	const worktreeLane = lane(page, "W");
	await commitRow(page, commitTitle).click();
	const file = worktreeLane.getByTestId("file-list-item").filter({ hasText: fileName });
	await expect(file).toBeVisible();
	return file;
}

/** Asserts W.txt left the worktree's commit and is uncommitted in the worktree only. */
async function expectUncommittedInWorktree(worktree: string, localClone: string) {
	await expect.poll(() => git(worktree, ["ls-tree", "--name-only", "W"])).not.toContain("W.txt");
	expect(git(worktree, ["log", "--format=%s", "-1", "W"])).toBe("W: first commit");
	expect(git(worktree, ["status", "--porcelain"]), "the change is uncommitted").toContain("W.txt");
	expect(git(localClone, ["status", "--porcelain"]), "the main checkout is untouched").toBe("");
}

test("uncommits a worktree commit from its menu", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	const parent = git(worktree, ["rev-parse", "W~1"]);
	await openWorkspace(page);

	await commitRow(page, "W: first commit").click({ button: "right" });
	await page.getByTestId("commit-row-context-menu-uncommit-menu-btn").click();

	await expect.poll(() => git(worktree, ["rev-parse", "W"])).toBe(parent);
	expect(git(worktree, ["status", "--porcelain"]), "the change is uncommitted").toContain("W.txt");
	expect(git(localClone, ["status", "--porcelain"]), "the main checkout is untouched").toBe("");
});

test("uncommits a worktree commit's file from its context menu", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openWorkspace(page);

	const file = await committedFile(page, "W: first commit", "W.txt");
	await file.click({ button: "right" });
	await page.getByRole("button", { name: "Uncommit changes" }).click();

	await expectUncommittedInWorktree(worktree, localClone);
});

test("uncommits a worktree commit's file by dragging it to uncommitted", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openWorkspace(page);

	const file = await committedFile(page, "W: first commit", "W.txt");
	// The worktree has no uncommitted changes, so the list shows its placeholder. The
	// dropzone's overlay covers it once the drag starts.
	await dragAndDropByLocator(page, file, lane(page, "W").getByText("No uncommitted changes"), {
		force: true,
	});

	await expectUncommittedInWorktree(worktree, localClone);
});

/** The `runHooks` flag of each push the app sends to the backend, in order. */
function recordPushHooks(page: Page): boolean[] {
	const runHooks: boolean[] = [];
	page.on("request", (request) => {
		if (request.url().endsWith("/workspace_branch_and_ancestors_push")) {
			runHooks.push((request.postDataJSON() as { runHooks: boolean }).runHooks);
		}
	});
	return runHooks;
}

test("pushes a worktree branch with the branches it rests on", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const remote = gitbutler.pathInWorkdir("remote-project");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openWorkspace(page);

	const pushButton = lane(page, "W").getByTestId("stack-push-button");
	await expect(pushButton).toBeEnabled();
	await pushButton.click();

	await expect
		.poll(() => git(remote, ["branch", "--list", "W", "B", "A"]).split("\n").length, {
			message: "Expected W and the stack branches beneath it on the remote",
		})
		.toBe(3);
	expect(git(remote, ["rev-parse", "W"])).toBe(git(localClone, ["rev-parse", "W"]));
	await expect(pushButton).toBeDisabled();
});

test("creates a PR for a worktree branch", async ({ page, gitbutler, fakeGithub }) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript(FIXTURE);
	// The fake GitHub serves pushes into the fixture's remote.
	git(gitbutler.pathInWorkdir("remote-project"), ["config", "http.receivepack", "true"]);
	git(gitbutler.pathInWorkdir("local-clone"), [
		"remote",
		"set-url",
		"origin",
		server.repositoryUrl,
	]);
	await storeFakeGitHubEnterprisePat(page, server);
	await openWorkspace(page);

	await lane(page, "W").getByTestId("create-review-button").click();
	await expect(lane(page, "W").getByTestId("create-review-box")).toBeVisible();
	await page.getByTestId("create-review-box-create-button").click();

	await expect
		.poll(() => server.getReviews().map((review) => review.head.ref), {
			message: "Expected a PR for the worktree branch",
		})
		.toEqual(["W"]);
});

test("creates a worktree from the new branch modal", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	await openWorkspace(page);

	await page.getByTestId("chrome-header-create-branch-button").click();
	const modal = page.getByTestId("create-new-branch-modal");
	await modal.locator("#new-branch-name-input").fill("fresh");
	await modal.locator('label[for="new-worktree"]').click();
	await modal.getByTestId("confirm-submit").click();

	await expect(lane(page, "fresh")).toBeVisible();
	const worktrees = git(gitbutler.pathInWorkdir("local-clone"), ["worktree", "list"]);
	expect(worktrees, "the branch is checked out in a new worktree").toMatch(/\[fresh\]/);
});

test("archiving a worktree removes its lane", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	await worktreeLane.getByRole("button", { name: "Open context menu" }).first().click();
	await page.getByRole("button", { name: "Archive worktree" }).click();

	await expect(worktreeLane).toHaveCount(0);
	await expect(lane(page, "C")).toBeVisible();
});

test("pushes a worktree branch without running hooks", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	await openWorkspace(page);
	// Hooks run in the main checkout, so only the stack branch's push may run them.
	const projectId = page.url().split("/")[3];
	await page.evaluate(
		(id) => localStorage.setItem(`projectRunCommitHooks_${id}`, "true"),
		projectId,
	);
	await page.reload();
	const runHooks = recordPushHooks(page);

	// The bottom branch's button pushes without asking about the branches above it.
	await lane(page, "C").getByTestId("stack-push-button").last().click();
	await expect.poll(() => runHooks).toEqual([true]);
	await lane(page, "W").getByTestId("stack-push-button").click();
	await expect.poll(() => runHooks).toEqual([true, false]);
});

test("keeps a worktree's files out of the main checkout's actions", async ({ page, gitbutler }) => {
	const pageErrors: Error[] = [];
	page.on("pageerror", (error) => pageErrors.push(error));
	await gitbutler.runScript(FIXTURE);
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	await expect(worktreeLane).toBeVisible();
	writeToFile(gitbutler.pathInWorkdir("worktree-W", "W.txt"), "W\nchanged\n");
	const file = worktreeLane.getByTestId("file-list-item").filter({ hasText: "W.txt" });
	await expect(file).toBeVisible();

	// Stashing and absorbing only know the main checkout.
	await file.click({ button: "right" });
	await expect(page.getByRole("button", { name: /^Open in / })).toBeVisible();
	await expect(page.getByRole("button", { name: "Stash into branch…" })).toHaveCount(0);
	await expect(page.getByTestId("file-list-item-context-menu__absorb")).toHaveCount(0);
	await page.keyboard.press("Escape");

	// A line click in the worktree's diff selects in the worktree, not the main checkout.
	await file.click();
	await page
		.locator('[data-testid="hunk-count-column"][data-is-delta-line="true"]')
		.first()
		.click();

	expect(pageErrors).toEqual([]);
	expect(git(localClone, ["status", "--porcelain"])).toBe("");
});

test("discards a worktree file's changes in that worktree", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	// The main checkout has a W.txt of its own, which discarding must leave alone.
	writeToFile(gitbutler.pathInWorkdir("local-clone", "W.txt"), "main\n");
	await openWorkspace(page);

	writeToFile(`${worktree}/W.txt`, "W\nchanged\n");
	const file = lane(page, "W").getByTestId("file-list-item").filter({ hasText: "W.txt" });
	await file.click({ button: "right" });
	await page.getByTestId("file-list-item-context-menu__discard-changes").click();
	await page.getByTestId("discard-file-changes-confirmation-modal-discard").click();

	await expect.poll(() => git(worktree, ["status", "--porcelain"])).toBe("");
	const mainFile = gitbutler.pathInWorkdir("local-clone", "W.txt");
	expect(existsSync(mainFile), "the main checkout's file is still there").toBe(true);
	expect(readFileSync(mainFile, "utf8"), "the main checkout's file is untouched").toBe("main\n");
});

test("discards a worktree file's hunk in that worktree", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	const mainFile = gitbutler.pathInWorkdir("local-clone", "W.txt");
	writeToFile(mainFile, "main\n");
	await openWorkspace(page);

	writeToFile(`${worktree}/W.txt`, "W\nchanged\n");
	await lane(page, "W").getByTestId("file-list-item").filter({ hasText: "W.txt" }).click();
	await page
		.getByTestId("unified-diff-view")
		.locator('[data-testid="hunk-count-column"]')
		.first()
		.click({ button: "right" });
	await page.getByTestId("hunk-context-menu-discard-change").click();

	await expect.poll(() => git(worktree, ["status", "--porcelain"])).toBe("");
	expect(existsSync(mainFile), "the main checkout's file is still there").toBe(true);
	expect(readFileSync(mainFile, "utf8"), "the main checkout's file is untouched").toBe("main\n");
});

test("removing a worktree deletes its folder and its lane", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	let removals = 0;
	page.on("request", (request) => {
		if (request.url().endsWith("/worktree_remove")) removals++;
	});
	await openWorkspace(page);

	const worktreeLane = lane(page, "W");
	await worktreeLane.getByRole("button", { name: "Open context menu" }).first().click();
	// The worktree's folder can be found from its lane.
	await expect(page.getByRole("button", { name: /^Show in / })).toBeVisible();
	await expect(page.getByRole("button", { name: "Copy path" })).toBeVisible();
	await page.getByRole("button", { name: "Remove worktree…" }).click();
	await page.getByRole("button", { name: "Remove", exact: true }).dblclick();

	await expect(worktreeLane).toHaveCount(0);
	expect(removals, "a double-click removes the worktree once").toBe(1);
	expect(existsSync(worktree), "the worktree's folder is deleted").toBe(false);
	expect(git(gitbutler.pathInWorkdir("local-clone"), ["rev-parse", "--verify", "W"])).not.toBe("");
});

test("settings drop a worktree removed outside the app", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	await openWorkspace(page);

	await clickByTestId(page, "chrome-sidebar-project-settings-button");
	const settings = page.getByTestId("project-settings-modal");
	await settings.getByRole("button", { name: "Worktrees" }).click();
	await expect(settings.getByRole("button", { name: "Archive", exact: true })).toBeVisible();

	const worktree = gitbutler.pathInWorkdir("worktree-W");
	git(gitbutler.pathInWorkdir("local-clone"), ["worktree", "remove", worktree]);

	await expect(settings.getByText("No active worktrees.")).toBeVisible();
});

test("commits onto a detached worktree's HEAD", async ({ page, gitbutler }) => {
	await gitbutler.runScript(FIXTURE);
	const worktree = gitbutler.pathInWorkdir("worktree-W");
	git(worktree, ["checkout", "--detach"]);
	writeToFile(`${worktree}/detached.txt`, "detached\n");
	git(worktree, ["add", "detached.txt"]);
	git(worktree, ["commit", "-m", "Detached: first commit"]);
	const branchTip = git(worktree, ["rev-parse", "W"]);
	await openWorkspace(page);

	// Its unnamed top segment goes by the worktree's name.
	const worktreeLane = lane(page, "worktree-W");
	await expect(worktreeLane).toBeVisible();
	writeToFile(`${worktree}/second.txt`, "second\n");
	await expect(
		worktreeLane.getByTestId("file-list-item").filter({ hasText: "second.txt" }),
	).toBeVisible();
	await worktreeLane.getByTestId("start-commit-button").click();
	await fillByTestId(page, "commit-drawer-title-input", "Detached: second commit");
	await page.getByTestId("commit-drawer-action-button").click();

	await expect
		.poll(() => git(worktree, ["log", "-1", "--format=%s", "HEAD"]))
		.toBe("Detached: second commit");
	expect(git(worktree, ["rev-parse", "W"]), "the branch stays where it was").toBe(branchTip);
	expect(git(worktree, ["status", "--porcelain"])).toBe("");
});

test("merges a worktree branch's PR", async ({ page, gitbutler }) => {
	const review = forgeReview(42, "W");
	await mockForge(page, {
		forge_info: gitlabForgeInfo(),
		list_reviews: [review],
		get_review: review,
		get_review_merge_status: mergeStatus({ isMergeable: true }),
		get_review_base_repo_url: null,
		get_repo_info: repoInfo(),
	});
	const merges: unknown[] = [];
	await page.route("**/merge_review", async (route) => {
		merges.push(route.request().postDataJSON());
		await route.fulfill({
			status: 200,
			contentType: "application/json",
			body: JSON.stringify({ type: "success", subject: null }),
		});
	});
	await gitbutler.runScript(FIXTURE);
	// A PR's branch has been pushed.
	git(gitbutler.pathInWorkdir("worktree-W"), ["push", "-u", "origin", "W"]);
	await gitbutler.runScript("cache-forge-review.sh", ["local-clone", "42", "W", "master"]);
	await openWorkspace(page);

	await lane(page, "W").getByTestId("branch-header").first().click();
	const mergeButton = page.getByTestId("pr-merge-button");
	await expect(mergeButton).toBeEnabled();
	await mergeButton.click();

	await expect.poll(() => merges).toHaveLength(1);
	expect(JSON.stringify(merges[0])).toContain("42");
});

/** Drags `branchName`'s header onto the topmost branch slot of `targetLane`. */
async function dragBranchToTopOf(page: Page, branchName: string, targetLane: Locator) {
	const header = page.locator(
		`[data-testid="branch-header"][data-testid-branch-header="${branchName}"]`,
	);
	await header.hover();
	const box = await header.boundingBox();
	if (!box) throw new Error(`Branch header ${branchName} has no bounding box`);
	// The branch's drag starts on movement, which renders the lanes' top dropzones.
	await page.mouse.down();
	await page.mouse.move(box.x + 16, box.y + 16);
	await nextFrame(page);
	const target = targetLane.getByTestId("BranchListInsertionDropzone").first();
	await target.hover({ force: true, position: { x: 120, y: 6 } });
	await nextFrame(page);
	await page.mouse.up();
}

async function nextFrame(page: Page) {
	await page.evaluate(
		async () => await new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
	);
}

test.describe("moving branches between the workspace and a worktree", () => {
	// branch1 is applied as a stack; the worktree has W on top of W-base.
	const STACKS_FIXTURE = "project-with-stacks-and-worktree.sh";

	test("moves a stack's branch into a worktree", async ({ page, gitbutler }) => {
		await gitbutler.runScript(STACKS_FIXTURE);
		const worktree = gitbutler.pathInWorkdir("worktree-W");
		const wTip = git(worktree, ["rev-parse", "W"]);
		await openWorkspace(page);

		await dragBranchToTopOf(page, "branch1", lane(page, "W"));

		await expect
			.poll(() => git(worktree, ["branch", "--show-current"]), {
				message: "a branch placed above what the worktree has checked out becomes its checkout",
			})
			.toBe("branch1");
		// Throws unless branch1 now builds on W.
		git(worktree, ["merge-base", "--is-ancestor", wTip, "branch1"]);
		// The worktree's lane is named after its top branch now, and no stack shows branch1.
		await expect(lane(page, "branch1")).toHaveCount(1);
		await expect(lane(page, "branch1")).toContainText("worktree");
	});

	test("moves a worktree's branch into the workspace", async ({ page, gitbutler }) => {
		await gitbutler.runScript(STACKS_FIXTURE);
		const worktree = gitbutler.pathInWorkdir("worktree-W");
		const localClone = gitbutler.pathInWorkdir("local-clone");
		const stackTip = git(localClone, ["rev-parse", "branch1"]);
		await openWorkspace(page);

		await dragBranchToTopOf(page, "W", lane(page, "branch1"));

		await expect
			.poll(() => git(worktree, ["branch", "--show-current"]), {
				message: "the worktree is left on the branch W was based on",
			})
			.toBe("W-base");
		// Throws unless W now builds on the stack's branch.
		git(localClone, ["merge-base", "--is-ancestor", stackTip, "W"]);
	});

	test("keeps a worktree's only branch", async ({ page, gitbutler }) => {
		await gitbutler.runScript(STACKS_FIXTURE);
		const worktree = gitbutler.pathInWorkdir("worktree-W");
		await openWorkspace(page);

		await dragBranchToTopOf(page, "W", lane(page, "branch1"));
		await expect.poll(() => git(worktree, ["branch", "--show-current"])).toBe("W-base");

		// W-base is all the worktree has left, so its lane refuses to give it up.
		await dragBranchToTopOf(page, "W-base", lane(page, "W"));
		expect(git(worktree, ["branch", "--show-current"])).toBe("W-base");
		await expect(lane(page, "W-base")).toBeVisible();
	});
});
