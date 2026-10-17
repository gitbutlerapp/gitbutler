// The update-from-remote flow applied for real with every answer, asserting the
// resulting git history.

import { expect, test } from "../test.ts";

test.describe("update from remote", () => {
	// Each test launches Electron and runs a real integration; 30s is not enough.
	test.describe.configure({ timeout: 180_000 });
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("a rewritten branch says so in its row and has nothing to bring in", async ({
		app,
		repo,
	}) => {
		await repo.rewriteBranch1Tip();

		// Head info prunes the remote twin, so nothing is new: the word carries the state.
		await expect(app.branch("branch1")).toShow("Rewritten");
		await expect(app.incomingCommits).toHaveCount(0);
		await expect(app.updateButton("branch1")).toHaveCount(0);
	});

	test("the incoming chip lists their new commits", async ({ app, repo }) => {
		await repo.divergeBranch1();

		await app.getByRole("button", { name: "Show 2 incoming commits from origin/branch1" }).click();

		await expect(app.getByText("Rework the parser entry point")).toBeVisible();
		await expect(app.getByText("Document the reworked entry point")).toBeVisible();
	});

	test("the preview of their additions names the conflict and a plain push", async ({
		app,
		repo,
	}) => {
		await repo.divergeBranch1();
		const update = await app.openBranchUpdate("branch1");

		await expect(update.dialog).toContainText("origin/branch1 has 2 new commits.");
		await expect(update.dialog).toContainText("1 commit will need conflict resolution.");
		await expect(update.dialog).toContainText("Push afterwards to publish.");
	});

	test("the preview of a rewrite with additions asks for a force push", async ({ app, repo }) => {
		await repo.divergeBoth();
		const update = await app.openBranchUpdate("branch1");

		await expect(update.dialog).toContainText(
			"origin/branch1 has 1 new commit, and a different version of one of yours.",
		);
		await expect(update.dialog).toContainText(
			"Rewrites the branch; force push afterwards to publish.",
		);
	});

	test("combine both puts their new commits under your work", async ({ app, repo }) => {
		await repo.divergeBranch1();
		const update = await app.openBranchUpdate("branch1");

		await update.apply("Combine both");
		await expect(update.button("Push")).toBeVisible();
		await update.finish();

		await expect(app.incomingCommits).toHaveCount(0);
		// Your commit, rebased onto their rework, conflicts, and its subject says so.
		expect(repo.subjects("branch1")).toStartWith([
			"[conflict] branch1: second commit",
			"Document the reworked entry point",
			"Rework the parser entry point",
			"branch1: first commit",
		]);
	});

	test("unticking an incoming commit leaves it out of the integration", async ({ app, repo }) => {
		await repo.divergeBranch1();
		const update = await app.openBranchUpdate("branch1");

		await update.leaveOut("Document the reworked entry point");
		await update.apply("Combine both");
		await update.finish();

		expect(repo.subjects("branch1")).toStartWith([
			"[conflict] branch1: second commit",
			"Rework the parser entry point",
			"branch1: first commit",
		]);
	});

	test("combine both never lands a rewritten commit twice", async ({ app, repo }) => {
		await repo.divergeBoth();
		const update = await app.openBranchUpdate("branch1");

		await update.apply("Combine both");
		await update.finish("Force push");

		expect(repo.subjects("branch1")).toStartWith([
			"Reworded locally",
			"Add upstream notes",
			"branch1: first commit",
		]);
		expect(
			repo.subjects("branch1").filter((subject) => subject.endsWith("Reworded locally")),
		).toHaveLength(1);
		expect(repo.remoteTip("branch1")).toBe(repo.tip("branch1"));
	});

	test("keep mine force-pushes the rewritten branch over their additions", async ({
		app,
		repo,
	}) => {
		await repo.divergeBoth();
		const update = await app.openBranchUpdate("branch1");

		await update.apply("Keep mine");
		await update.finish();

		expect(repo.remoteTip("branch1")).toBe(repo.tip("branch1"));
	});

	test("keep mine with additions force-pushes the branch as it is", async ({ app, repo }) => {
		await repo.divergeBranch1();
		const update = await app.openBranchUpdate("branch1");

		await update.apply("Keep mine");
		await update.finish();

		expect(repo.subjects("branch1")).toStartWith([
			"branch1: second commit",
			"branch1: first commit",
		]);
		expect(repo.remoteTip("branch1")).toBe(repo.tip("branch1"));
	});

	test("take theirs replaces the branch with the remote", async ({ app, repo }) => {
		await repo.divergeBranch1();
		const update = await app.openBranchUpdate("branch1");

		await update.apply("Take theirs");
		await update.finish();

		expect(repo.subjects("branch1")).toStartWith([
			"Document the reworked entry point",
			"Rework the parser entry point",
			"branch1: first commit",
		]);
	});
});
