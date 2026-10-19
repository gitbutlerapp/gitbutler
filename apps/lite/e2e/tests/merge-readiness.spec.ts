import { mergeReadiness } from "../merge-readiness-fixture.ts";
import { expect, test } from "../test.ts";

test.use({
	scenario: "project-in-single-branch-three-branch-stack.sh",
	viewport: { width: 1440, height: 900 },
	forge: mergeReadiness(),
});

test("pending reviews and an open conversation block the merge", async ({ app }) => {
	await app.selectBranch("C");

	await expect(app.checklist).toContainText("Blocked on review");
	await expect(app.checklist).toContainText("1 unresolved conversation");
	await expect(app.mergeButton).toBeDisabled();
});

test.describe("with failing checks", () => {
	test.use({ forge: mergeReadiness({ failing: true }) });

	test("the merge button says what holds it up", async ({ app }) => {
		await app.selectBranch("C");

		await expect(app.checklist).toContainText("Checks failing");
		await app.mergeButton.hover();
		await expect(app).toShow("4 reviews pending · Checks failed");
	});
});

test("moves a comment's verdict heading into its byline", async ({ app }) => {
	await app.selectBranch("C");
	const comment = app.reviewComment(21);

	await expect(comment).toShow("Changes recommended");
	await expect(comment).toContainText("Please review the remaining edge cases.");
	await expect(comment.getByRole("heading")).toHaveCount(0);
});

test.describe("with everything approved and resolved", () => {
	test.use({ forge: mergeReadiness({ clear: true }) });

	test("the checklist says the branch is ready to merge", async ({ app }) => {
		await app.selectBranch("C");

		await expect(app.checklist).toContainText("Ready to merge");
		await expect(app.checklist).toContainText("Conversations resolved");
		await expect(app.mergeButton).toBeEnabled();
	});

	test("keeps a pull request's to-dos across a reload", async ({ app }) => {
		await app.selectBranch("C");
		await app.addTodo("Check release notes");
		await app.addTodo("Try keyboard navigation");
		await app.tickTodo("Check release notes");

		await app.reload();

		await expect(app.todo("Check release notes")).toBeChecked();
		await expect(app.todo("Try keyboard navigation")).not.toBeChecked();
	});

	test("an open to-do doesn't block the merge", async ({ app }) => {
		await app.selectBranch("C");

		await app.addTodo("Check release notes");

		await expect(app.checklist).toContainText("0 of 1 to-dos");
		await expect(app.mergeButton).toBeEnabled();
	});

	test("removes a to-do", async ({ app }) => {
		await app.selectBranch("C");
		await app.addTodo("Check release notes");

		await app.removeTodo("Check release notes");

		await expect(app.todos).toHaveCount(0);
	});

	test("a branch keeps its to-dos before its pull request and across a rename", async ({ app }) => {
		await app.selectBranch("B");
		await app.branchTab("Pull Request").click();
		await app.addTodo("Write the description");

		await app.renameBranch("B", "B-renamed");
		await app.selectBranch("B-renamed");
		await app.branchTab("Pull Request").click();

		await expect(app.todo("Write the description")).toBeVisible();
	});

	test("to-dos belong to their own branch", async ({ app }) => {
		await app.selectBranch("C");
		await app.addTodo("Check release notes");

		await app.selectBranch("B");
		await app.branchTab("Pull Request").click();

		await expect(app.todos).toHaveCount(0);
	});
});
