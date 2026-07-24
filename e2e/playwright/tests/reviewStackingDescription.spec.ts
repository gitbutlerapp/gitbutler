import { assertGitConfigValue } from "../src/branch.ts";
import { mergeStatus, mockForge, repoInfo } from "../src/forge.ts";
import { applyUpstream, getButlerPort, openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	dragAndDropByLocator,
	stack,
	textEditorFillByTestId,
	waitForTestId,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import type { FakeGitHubReview, FakeGitHubServer } from "../src/fakeGithub.ts";

const FOOTER_TOP = "<!-- GitButler Footer Boundary Top -->";
const FOOTER_BOTTOM = "<!-- GitButler Footer Boundary Bottom -->";
const POLICY_KEY = "gitbutler.reviewStackingDescription";

type PushOutcome = {
	reviewSync:
		| { status: "notNeeded" }
		| { status: "succeeded" }
		| { status: "failed"; message: string };
};

test("review stack descriptions follow the per-project policy", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3", "branch4");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);
	await combineBranchesIntoStack(page);
	await gitbutler.runScript("push-branch.sh", ["branch4"]);

	await publishReview(page, "branch1", "Description for branch1");
	await expect(page.getByText("PR #42 created successfully")).toBeVisible();
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");

	await expectReviews(server, 2, (reviews) => {
		expectBottomFooter(reviews[0], "Description for branch1", "part 1 of 2");
		expectBottomFooter(reviews[1], "Description for branch2", "part 2 of 2");
		expectStackOrder(reviews, [43, 42]);
	});

	await setDescriptionPolicy(page, gitbutler, "top");
	await publishReview(page, "branch3", "Description for branch3");

	await expectReviews(server, 3, (reviews) => {
		for (const [index, review] of reviews.entries()) {
			expect(review.body).toMatch(new RegExp(`^${escapeRegExp(FOOTER_TOP)}`));
			expect(review.body).toContain(`part ${index + 1} of 3`);
			expect(review.body).toMatch(new RegExp(`Description for branch${index + 1}$`));
		}
		expectStackOrder(reviews, [44, 43, 42]);
	});

	await setDescriptionPolicy(page, gitbutler, "disabled");
	await publishReview(page, "branch4", "Description for branch4");

	await expectReviews(server, 4, (reviews) => {
		for (const [index, review] of reviews.entries()) {
			expect(review.body).toBe(`Description for branch${index + 1}`);
			expect(review.body).not.toContain(FOOTER_TOP);
			expect(review.body).not.toContain(FOOTER_BOTTOM);
		}
		expect(reviews.map((review) => review.base.ref)).toEqual([
			"master",
			"branch1",
			"branch2",
			"branch3",
		]);
	});
});

test("review creation stays successful when stack synchronization fails", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	await applyUpstream(gitbutler, "branch1");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	server.setReviewUpdatesFailing(true);
	await publishReview(page, "branch1", "Durable review body");

	await expect(page.getByText("PR #42 created successfully")).toBeVisible();
	expect(server.getReview(42)?.head.ref).toBe("branch1");
	expect(server.getReview(42)?.body).toBe("Durable review body");
	await expect(
		page
			.getByTestId("toast-info-message")
			.filter({ hasText: "Pull request created with incomplete stack information" }),
	).toHaveCount(0);

	writeFileSync(
		gitbutler.pathInWorkdir("local-clone/d_file"),
		"branch1 change after review creation\n",
		{ flag: "a" },
	);
	const branch1Stack = stack(page, "branch1");
	await branch1Stack.getByTestId("start-commit-button").click();
	await page.getByTestId("commit-drawer-title-input").fill("branch1: post-review change");
	await page.getByTestId("commit-drawer-action-button").click();
	await branch1Stack.getByTestId("stack-push-button").click();

	await expect(page.getByText("Pushed branch1 successfully")).toBeVisible();
	await expect(
		page
			.getByTestId("toast-info-message")
			.filter({ hasText: "Push succeeded with incomplete review synchronization" }),
	).toHaveCount(0);
});

test("review creation distinguishes a local branch from its differently named remote head", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1");

	const worktree = gitbutler.pathInWorkdir("local-clone");
	const branchTip = git(worktree, ["rev-parse", "refs/heads/branch1"]);
	git(gitbutler.pathInWorkdir("remote-project"), [
		"update-ref",
		"refs/heads/users/alice/feature",
		branchTip,
	]);
	git(worktree, ["update-ref", "refs/remotes/origin/users/alice/feature", branchTip]);
	git(worktree, ["config", "branch.branch1.remote", "origin"]);
	git(worktree, ["config", "branch.branch1.merge", "refs/heads/users/alice/feature"]);

	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);
	await publishReview(page, "branch1", "Review with renamed upstream");

	await expect(page.getByText("PR #42 created successfully")).toBeVisible();
	expect(server.getReview(42)?.head.ref).toBe("users/alice/feature");
	expect(server.getReview(42)?.base.ref).toBe("master");
});

test("local moves do not update reviews and partial pushes synchronize the complete review stack", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3", "branch4");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	for (const [branch, parent, remainingStacks] of [
		["branch2", "branch1", 3],
		["branch3", "branch2", 2],
	] as const) {
		await dragAndDropByLocator(
			page,
			branchHeaders.filter({ hasText: branch }),
			branchHeaders.filter({ hasText: parent }),
			{ force: true, position: { x: 120, y: -10 } },
		);
		await expect(stack(page)).toHaveCount(remainingStacks);
	}
	await gitbutler.runScript("push-branch.sh", ["branch3"]);

	await publishReview(page, "branch1", "Description for branch1");
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");
	await publishReview(page, "branch3", "Description for branch3");
	await publishReview(page, "branch4", "Description for branch4");
	await expect(page.getByText("PR #45 created successfully")).toBeVisible();

	await expectReviews(server, 4, (reviews) => {
		expectStackMembership(reviews[0], [42, 43, 44], [45]);
		expectStackMembership(reviews[1], [42, 43, 44], [45]);
		expectStackMembership(reviews[2], [42, 43, 44], [45]);
		expectStackMembership(reviews[3], [], [42, 43, 44]);
		expect(reviews.map((review) => review.base.ref)).toEqual([
			"master",
			"branch1",
			"branch2",
			"master",
		]);
		expect(reviews[3]?.body).toBe("Description for branch4");
	});

	const reviewUpdatesBeforeMove = server.getReviewUpdateCount();
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, branchHeaders.filter({ hasText: "branch3" }), stackDropzone, {
		force: true,
		position: { x: 10, y: 10 },
	});
	await expect(stack(page)).toHaveCount(3);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "tearing off a branch must not contact the forge",
		})
		.toBe(reviewUpdatesBeforeMove);

	const beforeRemovedStackPush = reviewHistoryLengths(server, [42, 43, 44, 45]);
	writeFileSync(
		gitbutler.pathInWorkdir("local-clone/b_file"),
		"branch2 change after tearing off branch3\n",
		{ flag: "a" },
	);
	await gitbutler.runScript("commit-branch.sh", ["branch2", "branch2: change after tear-off"]);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await expectReviewHistoryDeltas(server, beforeRemovedStackPush, {
		42: 1,
		43: 1,
		44: 1,
		45: 0,
	});
	await expectReviews(server, 4, (reviews) => {
		expectStackMembership(reviews[0], [42, 43], [44, 45]);
		expectStackMembership(reviews[1], [42, 43], [44, 45]);
		expectStackMembership(reviews[2], [], [42, 43, 44, 45]);
		expect(reviews[2]?.body).toBe("Description for branch3");
		expectStackMembership(reviews[3], [], [42, 43, 44]);
		expect(reviews.map((review) => review.base.ref)).toEqual([
			"master",
			"branch1",
			"master",
			"master",
		]);
	});

	const reviewUpdatesBeforeReattach = server.getReviewUpdateCount();
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch3" }),
		branchHeaders.filter({ hasText: "branch2" }),
		{ force: true, position: { x: 120, y: -10 } },
	);
	await expect(stack(page)).toHaveCount(2);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "local branch moves must not contact the forge",
		})
		.toBe(reviewUpdatesBeforeReattach);

	const beforeMiddlePush = reviewHistoryLengths(server, [42, 43, 44, 45]);
	writeFileSync(
		gitbutler.pathInWorkdir("local-clone/b_file"),
		"branch2 change after reattaching branch3\n",
		{ flag: "a" },
	);
	await gitbutler.runScript("commit-branch.sh", ["branch2", "branch2: change after reattach"]);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await expectReviewHistoryDeltas(server, beforeMiddlePush, {
		42: 1,
		43: 1,
		44: 1,
		45: 0,
	});

	await expectReviews(server, 4, (reviews) => {
		expectStackMembership(reviews[0], [42, 43, 44], [45]);
		expectStackMembership(reviews[1], [42, 43, 44], [45]);
		expectStackMembership(reviews[2], [42, 43, 44], [45]);
		expectStackMembership(reviews[3], [], [42, 43, 44]);
		expect(reviews.map((review) => review.base.ref)).toEqual([
			"master",
			"branch1",
			"branch2",
			"master",
		]);
		expect(reviews[3]?.body).toBe("Description for branch4");
	});

	const beforeTopPush = reviewHistoryLengths(server, [42, 43, 44, 45]);
	writeFileSync(
		gitbutler.pathInWorkdir("local-clone/c_file"),
		"branch3 change after publishing reviews\n",
		{ flag: "a" },
	);
	await gitbutler.runScript("commit-branch.sh", ["branch3", "branch3: post-review change"]);
	await gitbutler.runScript("push-branch.sh", ["branch3"]);
	await expectReviewHistoryDeltas(server, beforeTopPush, {
		42: 1,
		43: 1,
		44: 1,
		45: 0,
	});

	const reviews = server.getReviews();
	expectStackMembership(reviews[0], [42, 43, 44], [45]);
	expectStackMembership(reviews[1], [42, 43, 44], [45]);
	expectStackMembership(reviews[2], [42, 43, 44], [45]);
	expectStackMembership(reviews[3], [], [42, 43, 44]);
	expect(reviews.map((review) => review.base.ref)).toEqual([
		"master",
		"branch1",
		"branch2",
		"master",
	]);
	expect(reviews[3]?.body).toBe("Description for branch4");
});

test("creating a review in the middle synchronizes the complete review stack", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch2" }),
		branchHeaders.filter({ hasText: "branch1" }),
		{ force: true, position: { x: 120, y: -10 } },
	);
	await expect(stack(page)).toHaveCount(2);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);

	await publishReview(page, "branch1", "Description for branch1");
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");
	await expect(page.getByText("PR #43 created successfully")).toBeVisible();

	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch3" }),
		branchHeaders.filter({ hasText: "branch1" }),
		{ force: true, position: { x: 120, y: 0 } },
	);
	await expect(stack(page)).toHaveCount(1);

	const beforeUnreviewedPush = reviewHistoryLengths(server, [42, 43]);
	await gitbutler.runScript("push-branch.sh", ["branch3"]);
	await expectReviewHistoryDeltas(server, beforeUnreviewedPush, { 42: 0, 43: 0 });

	const beforeGapPush = reviewHistoryLengths(server, [42, 43]);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await expectReviewHistoryDeltas(server, beforeGapPush, { 42: 1, 43: 1 });
	await expectReviews(server, 2, (reviews) => {
		expect(reviews[0]?.base.ref).toBe("master");
		expect(reviews[1]?.base.ref).toBe("branch1");
		for (const review of reviews) {
			expectStackMembership(review, [42, 43], []);
			expect(review.body).not.toContain("#44");
		}
	});

	// Local branch2 may advance after the remote graph was made coherent. Its remote still
	// contains branch3, so publishing branch3 must include branch2 in the review stack.
	writeFileSync(
		gitbutler.pathInWorkdir("local-clone/b_file"),
		"local-only branch2 change before publishing branch3\n",
		{ flag: "a" },
	);
	await gitbutler.runScript("commit-branch.sh", [
		"branch2",
		"branch2: local-only change after stacking branch3",
	]);

	const beforeMiddleCreation = reviewHistoryLengths(server, [42, 43, 44]);
	await publishReview(page, "branch3", "Description for branch3");
	await expect(page.getByText("PR #44 created successfully")).toBeVisible();
	await expectReviewHistoryDeltas(server, beforeMiddleCreation, { 42: 1, 43: 1, 44: 1 });

	await expectReviews(server, 3, (reviews) => {
		const [reviewA, reviewB, reviewC] = reviews;
		expect(reviewA?.base.ref).toBe("master");
		expect(reviewC?.base.ref).toBe("branch1");
		expect(reviewB?.base.ref).toBe("branch3");
		for (const review of reviews) {
			expectStackMembership(review, [42, 43, 44], []);
		}
	});
});

test("reordering reviewed refs updates every target after a partial push", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	for (const [branch, parent, remainingStacks] of [
		["branch2", "branch1", 2],
		["branch3", "branch2", 1],
	] as const) {
		await dragAndDropByLocator(
			page,
			branchHeaders.filter({ hasText: branch }),
			branchHeaders.filter({ hasText: parent }),
			{ force: true, position: { x: 120, y: -10 } },
		);
		await expect(stack(page)).toHaveCount(remainingStacks);
	}
	await gitbutler.runScript("push-branch.sh", ["branch3"]);

	await publishReview(page, "branch1", "Description for branch1");
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");
	await publishReview(page, "branch3", "Description for branch3");
	await expect(page.getByText("PR #44 created successfully")).toBeVisible();
	await expectBranchHeaderOrder(page, ["branch3", "branch2", "branch1"]);

	const beforeReorder = server.getReviewUpdateCount();
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch2" }),
		branchHeaders.filter({ hasText: "branch3" }),
		{ force: true, position: { x: 120, y: -10 } },
	);
	await expectBranchHeaderOrder(page, ["branch2", "branch3", "branch1"]);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "reordering reviewed refs locally must not contact the forge",
		})
		.toBe(beforeReorder);

	const beforePartialPush = reviewHistoryLengths(server, [42, 43, 44]);
	await gitbutler.runScript("push-branch.sh", ["branch3"]);
	await expectReviewHistoryDeltas(server, beforePartialPush, { 42: 1, 43: 1, 44: 1 });
	await expectReviews(server, 3, (reviews) => {
		expect(reviews.map((review) => review.base.ref)).toEqual(["master", "branch3", "branch1"]);
		for (const review of reviews) {
			expectStackMembership(review, [42, 43, 44], []);
		}
		expectStackOrder(reviews, [43, 44, 42]);
	});

	const beforeTopPush = reviewHistoryLengths(server, [42, 43, 44]);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await expectReviewHistoryDeltas(server, beforeTopPush, { 42: 1, 43: 1, 44: 1 });
	await expectReviews(server, 3, (reviews) => {
		expect(reviews.map((review) => review.base.ref)).toEqual(["master", "branch3", "branch1"]);
		expectStackOrder(reviews, [43, 44, 42]);
	});
});

test("removing a reviewed middle ref is reflected after pushing the remaining stack", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	for (const [branch, parent, remainingStacks] of [
		["branch2", "branch1", 2],
		["branch3", "branch2", 1],
	] as const) {
		await dragAndDropByLocator(
			page,
			branchHeaders.filter({ hasText: branch }),
			branchHeaders.filter({ hasText: parent }),
			{ force: true, position: { x: 120, y: -10 } },
		);
		await expect(stack(page)).toHaveCount(remainingStacks);
	}
	await gitbutler.runScript("push-branch.sh", ["branch3"]);

	await publishReview(page, "branch1", "Description for branch1");
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");
	await publishReview(page, "branch3", "Description for branch3");
	await expect(page.getByText("PR #44 created successfully")).toBeVisible();

	const beforeRemoval = server.getReviewUpdateCount();
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, branchHeaders.filter({ hasText: "branch2" }), stackDropzone, {
		force: true,
		position: { x: 10, y: 10 },
	});
	await expect(stack(page)).toHaveCount(2);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "removing a reviewed middle ref locally must not contact the forge",
		})
		.toBe(beforeRemoval);

	const beforeRemainingStackPush = reviewHistoryLengths(server, [42, 43, 44]);
	await gitbutler.runScript("push-branch.sh", ["branch3"]);
	await expectReviewHistoryDeltas(server, beforeRemainingStackPush, { 42: 1, 43: 1, 44: 1 });
	await expectReviews(server, 3, (reviews) => {
		const [reviewA, reviewB, reviewC] = reviews;
		expect(reviewA?.base.ref).toBe("master");
		expect(reviewB?.base.ref).toBe("master");
		expect(reviewC?.base.ref).toBe("branch1");
		expectStackMembership(reviewA, [42, 44], [43]);
		expectStackMembership(reviewB, [], [42, 44]);
		expectStackMembership(reviewC, [42, 44], [43]);
		expect(reviewB?.body).toBe("Description for branch2");
	});
});

test("moving a reviewed ref between reviewed stacks continues after one stack update fails", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3", "branch4");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	for (const [branch, parent, remainingStacks] of [
		["branch2", "branch1", 3],
		["branch4", "branch3", 2],
	] as const) {
		await dragAndDropByLocator(
			page,
			branchHeaders.filter({ hasText: branch }),
			branchHeaders.filter({ hasText: parent }),
			{ force: true, position: { x: 120, y: -10 } },
		);
		await expect(stack(page)).toHaveCount(remainingStacks);
	}
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await gitbutler.runScript("push-branch.sh", ["branch4"]);

	await publishReview(page, "branch1", "Description for branch1");
	server.setListed(true);
	await publishReview(page, "branch2", "Description for branch2");
	await publishReview(page, "branch3", "Description for branch3");
	await publishReview(page, "branch4", "Description for branch4");
	await expect(page.getByText("PR #45 created successfully")).toBeVisible();

	const beforeMove = server.getReviewUpdateCount();
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch2" }),
		branchHeaders.filter({ hasText: "branch4" }),
		{ force: true, position: { x: 120, y: -10 } },
	);
	await expect(stack(page)).toHaveCount(2);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "moving a reviewed ref between stacks locally must not contact the forge",
		})
		.toBe(beforeMove);

	server.setFailingReviewUpdates([42]);
	const beforePush = reviewHistoryLengths(server, [42, 43, 44, 45]);
	await gitbutler.runScript("push-branch.sh", ["branch2"]);
	await expectReviewHistoryDeltas(server, beforePush, { 42: 1, 43: 1, 44: 1, 45: 1 });
	await expectReviews(server, 4, (reviews) => {
		const [reviewA, reviewB, reviewC, reviewD] = reviews;
		expect(reviews.map((review) => review.base.ref)).toEqual([
			"master",
			"branch4",
			"master",
			"branch3",
		]);
		// The configured failure leaves the first stack's previous footer intact,
		// while the remaining affected stack still converges.
		expectStackMembership(reviewA, [42, 43], [44, 45]);
		expectStackMembership(reviewB, [43, 44, 45], [42]);
		expectStackMembership(reviewC, [43, 44, 45], [42]);
		expectStackMembership(reviewD, [43, 44, 45], [42]);
		expectStackOrder([reviewB, reviewC, reviewD], [43, 45, 44]);
	});
});

test("CLI push reports succeeded, notNeeded, and failed review synchronization", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	await publishReview(page, "branch1", "Description for branch1");
	await expect(page.getByText("PR #42 created successfully")).toBeVisible();
	server.setListed(true);

	writeFileSync(gitbutler.pathInWorkdir("local-clone/a_file"), "successful sync push\n", {
		flag: "a",
	});
	await gitbutler.runScript("commit-branch.sh", ["branch1", "branch1: successful sync"]);
	const succeededPath = gitbutler.pathInWorkdir("succeeded-push.json");
	const beforeSucceeded = reviewHistoryLengths(server, [42]);
	await gitbutler.runScript("push-branch-json.sh", ["branch1", succeededPath]);
	expect(readPushOutcome(succeededPath).reviewSync).toEqual({ status: "succeeded" });
	await expectReviewHistoryDeltas(server, beforeSucceeded, { 42: 1 });

	const notNeededPath = gitbutler.pathInWorkdir("not-needed-push.json");
	const beforeNoOp = reviewHistoryLengths(server, [42]);
	await gitbutler.runScript("push-branch-json.sh", ["branch1", notNeededPath]);
	expect(readPushOutcome(notNeededPath).reviewSync).toEqual({ status: "notNeeded" });
	await expectReviewHistoryDeltas(server, beforeNoOp, { 42: 0 });

	server.setReviewUpdatesFailing(true);
	writeFileSync(gitbutler.pathInWorkdir("local-clone/a_file"), "failed sync push\n", {
		flag: "a",
	});
	await gitbutler.runScript("commit-branch.sh", ["branch1", "branch1: failed sync"]);
	const failedPath = gitbutler.pathInWorkdir("failed-push.json");
	const beforeFailed = reviewHistoryLengths(server, [42]);
	await gitbutler.runScript("push-branch-json.sh", ["branch1", failedPath]);
	const failed = readPushOutcome(failedPath).reviewSync;
	expect(failed.status).toBe("failed");
	if (failed.status !== "failed") {
		throw new Error(`Expected failed review synchronization, received ${failed.status}`);
	}
	expect(failed.message).toEqual(expect.any(String));
	expect(failed.message).not.toBe("");
	await expectReviewHistoryDeltas(server, beforeFailed, { 42: 1 });
});

test("a Git push failure does not start review synchronization", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		listed: false,
	});
	await gitbutler.runScript("project-with-stacks.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	mirrorFakeCredentialForCli(gitbutler);
	await applyUpstream(gitbutler, "branch1");
	await mockForge(page, {
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
	await openWorkspace(page);

	await publishReview(page, "branch1", "Description for branch1");
	await expect(page.getByText("PR #42 created successfully")).toBeVisible();
	server.setListed(true);
	writeFileSync(gitbutler.pathInWorkdir("local-clone/a_file"), "rejected Git push\n", {
		flag: "a",
	});
	await gitbutler.runScript("commit-branch.sh", ["branch1", "branch1: rejected push"]);

	const beforeFailedPush = server.getReviewUpdateCount();
	server.setGitPushesFailing(true);
	await expect(gitbutler.runScript("push-branch.sh", ["branch1"])).rejects.toThrow(
		"Command failed with exit code",
	);
	await expect
		.poll(() => server.getReviewUpdateCount(), {
			message: "review synchronization must not start after the Git push fails",
		})
		.toBe(beforeFailedPush);
});

async function combineBranchesIntoStack(page: Page) {
	const branchHeaders = page.getByTestId("branch-header");
	await expect(stack(page)).toHaveCount(4);

	for (const [branch, parent, remainingStacks] of [
		["branch2", "branch1", 3],
		["branch3", "branch2", 2],
		["branch4", "branch3", 1],
	] as const) {
		await dragAndDropByLocator(
			page,
			branchHeaders.filter({ hasText: branch }),
			branchHeaders.filter({ hasText: parent }),
			{ force: true, position: { x: 120, y: -10 } },
		);
		await expect(stack(page)).toHaveCount(remainingStacks);
	}
}

async function expectBranchHeaderOrder(page: Page, expected: string[]) {
	await expect
		.poll(
			async () =>
				await page
					.getByTestId("branch-header")
					.evaluateAll((headers) =>
						headers.map((header) => header.getAttribute("data-testid-branch-header")),
					),
		)
		.toEqual(expected);
}

function git(worktree: string, args: string[]) {
	return execFileSync("git", args, { cwd: worktree, encoding: "utf8" }).trim();
}

function readPushOutcome(path: string): PushOutcome {
	return JSON.parse(readFileSync(path, "utf8")) as PushOutcome;
}

async function publishReview(page: Page, branch: string, description: string) {
	const header = page.getByTestId("branch-header").filter({ hasText: branch });
	const headerWrapper = header.locator("..");
	await headerWrapper.getByTestId("create-review-button").click();
	await waitForTestId(page, "create-review-box");
	await textEditorFillByTestId(page, "create-review-box-description-input", description);
	await clickByTestId(page, "create-review-box-create-button");
	await expect(headerWrapper.getByTestId("create-review-button")).toHaveCount(0);
}

async function setDescriptionPolicy(page: Page, gitbutler: GitButler, policy: "top" | "disabled") {
	await clickByTestId(page, "chrome-sidebar-project-settings-button");
	await waitForTestId(page, "project-settings-modal");
	const select = page.getByTestId("review-stacking-description-select");
	await expect(select).toBeVisible();
	await select.scrollIntoViewIfNeeded();
	await select.click();
	await page
		.getByTestId(`review-stacking-description-option-${policy}`)
		.getByRole("button")
		.click();
	await assertGitConfigValue(POLICY_KEY, policy, gitbutler.pathInWorkdir("local-clone"));
	await page.keyboard.press("Escape");
	await expect(page.getByTestId("project-settings-modal")).toHaveCount(0);
}

async function expectReviews(
	server: FakeGitHubServer,
	count: number,
	assertions: (reviews: FakeGitHubReview[]) => void,
) {
	await expect
		.poll(
			() => {
				const reviews = server.getReviews();
				if (reviews.length !== count) {
					return `Expected ${count} reviews, received ${reviews.length}`;
				}
				try {
					assertions(reviews);
					return "ok";
				} catch (error) {
					return JSON.stringify({
						error: String(error),
						reviews: reviews.map(({ number, body, base, head }) => ({
							number,
							body,
							base: base.ref,
							head: head.ref,
						})),
					});
				}
			},
			{
				message: `Expected ${count} fake GitHub reviews with synchronized descriptions`,
			},
		)
		.toBe("ok");
}

function expectBottomFooter(review: FakeGitHubReview, description: string, part: string) {
	expect(review.body).toMatch(new RegExp(`^${description}`));
	expect(review.body).toContain(part);
	expect(review.body).toMatch(new RegExp(`${escapeRegExp(FOOTER_BOTTOM)}$`));
}

function expectStackOrder(reviews: FakeGitHubReview[], topToBase: number[]) {
	for (const review of reviews) {
		const body = review.body ?? "";
		const positions = topToBase.map((number) => body.indexOf(`#${number}`));
		expect(positions.every((position) => position >= 0)).toBe(true);
		expect(positions).toEqual([...positions].sort((a, b) => a - b));
	}
}

function expectStackMembership(
	review: FakeGitHubReview,
	expectedReviewNumbers: number[],
	unrelatedReviewNumbers: number[],
) {
	const body = review.body ?? "";
	for (const number of expectedReviewNumbers) {
		expect(body, `review #${review.number} should include stack peer #${number}`).toContain(
			`#${number}`,
		);
	}
	for (const number of unrelatedReviewNumbers) {
		expect(
			body,
			`review #${review.number} should not include unrelated review #${number}`,
		).not.toContain(`#${number}`);
	}
}

function reviewHistoryLengths(server: FakeGitHubServer, reviews: number[]) {
	return Object.fromEntries(
		reviews.map((number) => [number, server.getReviewUpdateHistory(number).length]),
	);
}

async function expectReviewHistoryDeltas(
	server: FakeGitHubServer,
	before: Record<number, number>,
	expected: Record<number, number>,
) {
	await expect
		.poll(() =>
			Object.fromEntries(
				Object.keys(expected).map((number) => [
					number,
					server.getReviewUpdateHistory(Number(number)).length - (before[Number(number)] ?? 0),
				]),
			),
		)
		.toEqual(expected);
}

async function storeFakeGitHubEnterprisePat(page: Page, server: FakeGitHubServer) {
	const response = await page.request.post(
		`http://localhost:${getButlerPort()}/store_github_enterprise_pat`,
		{
			data: { host: server.apiBaseUrl, accessToken: "fake-token" },
		},
	);
	expect(response.ok()).toBe(true);
}

function mirrorFakeCredentialForCli(gitbutler: GitButler) {
	const credentialPath = gitbutler.pathInWorkdir("../config/git-credentials");
	const serverCredential = readFileSync(credentialPath, "utf8");
	const cliCredential = serverCredential.replace("development-", "com.gitbutler.app.dev-");
	writeFileSync(credentialPath, `${serverCredential.trimEnd()}\n${cliCredential}`);
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
