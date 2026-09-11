import { expect, test } from "../test.ts";
import { openMergeReadinessReview, setChecklistReviewState } from "../merge-readiness-fixture.ts";
import type { Page } from "@playwright/test";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

const checklistOf = (page: Page) => page.getByRole("region", { name: "Checklist", exact: true });
const mergeButtonOf = (page: Page) => page.getByRole("button", { name: "Merge", exact: true });
const selectBranch = (page: Page, name: string) =>
	page.getByRole("treeitem", { name, exact: true }).getByTitle(name, { exact: true }).click();

const addTodo = async (page: Page, label: string) => {
	const checklist = checklistOf(page);
	await checklist.getByRole("button", { name: "Add to-do", exact: true }).click();
	await checklist.getByRole("textbox", { name: "To-do" }).fill(label);
	await page.keyboard.press("ControlOrMeta+Enter");
};

test("says what holds up the merge, and when nothing does", async ({ appWindow, electronApp }) => {
	await openMergeReadinessReview(appWindow, electronApp);
	const checklist = checklistOf(appWindow);
	const merge = mergeButtonOf(appWindow);

	await expect(checklist).toContainText("Blocked on review");
	await expect(checklist).toContainText("1 unresolved conversation");
	await expect(merge).toBeDisabled();

	await setChecklistReviewState(electronApp, { failing: true });
	await appWindow.reload();
	await expect(checklist).toContainText("Checks failing");
	await merge.hover();
	await expect(
		appWindow.getByText("4 reviews pending · Checks failed", { exact: true }),
	).toBeVisible();

	await setChecklistReviewState(electronApp, { clear: true });
	await appWindow.reload();
	await expect(checklist).toContainText("Ready to merge");
	await expect(checklist).toContainText("Conversations resolved");
	await expect(merge).toBeEnabled();
});

test("moves a comment's verdict heading into its byline", async ({ appWindow, electronApp }) => {
	await openMergeReadinessReview(appWindow, electronApp);
	const comment = appWindow.locator("#review-comment-21");

	await expect(comment.getByText("Changes recommended", { exact: true })).toBeVisible();
	await expect(comment).toContainText("Please review the remaining edge cases.");
	await expect(comment.getByRole("heading")).toHaveCount(0);
});

test("keeps a pull request's to-dos across a reload", async ({ appWindow, electronApp }) => {
	await openMergeReadinessReview(appWindow, electronApp, { clear: true });
	const checklist = checklistOf(appWindow);

	await addTodo(appWindow, "Check release notes");
	await addTodo(appWindow, "Try keyboard navigation");
	const notes = checklist.getByRole("checkbox", { name: "Check release notes" });
	const keyboard = checklist.getByRole("checkbox", { name: "Try keyboard navigation" });
	await notes.click();
	await expect(notes).toBeChecked();
	await expect(checklist).toContainText("1 of 2 to-dos");
	await expect(mergeButtonOf(appWindow)).toBeEnabled();

	await appWindow.reload();
	await expect(notes).toBeChecked();
	await expect(keyboard).not.toBeChecked();

	await checklist.getByRole("button", { name: "Remove Check release notes", exact: true }).click();
	await expect(checklist.getByRole("checkbox")).toHaveCount(1);
});

test("keeps to-dos with the branch, before its pull request and across a rename", async ({
	appWindow,
	electronApp,
}) => {
	await openMergeReadinessReview(appWindow, electronApp, { clear: true });
	const checklist = checklistOf(appWindow);
	await addTodo(appWindow, "Check release notes");

	// A branch without a pull request yet keeps its own list, on the create form.
	await selectBranch(appWindow, "B");
	await appWindow.getByRole("button", { name: "Create pull request", exact: true }).click();
	await expect(checklist.getByRole("checkbox")).toHaveCount(0);
	await addTodo(appWindow, "Write the description");
	const description = checklist.getByRole("checkbox", { name: "Write the description" });
	await expect(description).toBeVisible();

	await selectBranch(appWindow, "B");
	await appWindow.keyboard.press("F2");
	const branchName = appWindow.getByRole("textbox", { name: "Branch name" });
	await branchName.fill("B-renamed");
	await branchName.press("Enter");
	await selectBranch(appWindow, "B-renamed");
	await appWindow.getByRole("button", { name: "Create pull request", exact: true }).click();
	await expect(description).toBeVisible();
});
