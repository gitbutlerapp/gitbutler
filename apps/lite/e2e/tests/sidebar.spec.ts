import type { App } from "../app.ts";
import { aReview } from "../forge.ts";
import { expect, test } from "../test.ts";

const review = aReview({ sourceBranch: "C" });

test.use({
	scenario: "project-in-single-branch-three-branch-stack.sh",
	forge: { reviews: [review] },
});

test.beforeEach(async ({ app }) => {
	const settings = await app.page.evaluate(() => window.lite.readGUISettings());
	await app.backend.stub({ readGUISettings: { ...settings, prNotifications: "loud" } });

	const notification = (id: string, author: string | null, snippet: string) => ({
		id,
		kind: "comment",
		review: review.number,
		reviewTitle: review.title,
		unitSymbol: review.unitSymbol,
		sourceBranch: review.sourceBranch,
		htmlUrl: review.htmlUrl,
		author,
		count: 1,
		snippet,
		at: review.modifiedAt,
		seen: false,
	});
	await app.page.evaluate(
		({ projectId, seen, inbox }) => {
			localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify(seen));
			localStorage.setItem(`pr_activity_inbox:v1:${projectId}`, JSON.stringify(inbox));
		},
		{
			projectId: app.projectId,
			seen: { [review.number]: review.createdAt },
			inbox: [
				notification("comment-1", null, "Please take a look"),
				notification("bot-1", "copilot-pull-request-reviewer", "Bot review"),
			],
		},
	);
	await app.reload();
});

const notifications = (app: App, unread: number) =>
	app.getByRole("button", { name: `Notifications, ${unread} unread` });

const openNotifications = async (app: App) => {
	await notifications(app, 2).click();
	const types = app.getByRole("group", { name: "Notification type" });
	return {
		panel: app.getByRole("dialog").filter({ has: types }),
		type: (name: string) => types.getByRole("button", { name, exact: true }),
	};
};

test("keeps unread PR activity off the Workspace tab", async ({ app }) => {
	await expect(notifications(app, 2)).toBeVisible();
	// No unread count rides along on the tab's name.
	await expect(app.pageTab("Workspace")).toHaveText("Workspace");
});

test("shows people's activity first and agents' under their own type", async ({ app }) => {
	const { panel, type } = await openNotifications(app);
	await expect(panel).toContainText("Please take a look");
	await expect(panel).not.toContainText("Bot review");

	await type("Agents (1)").click();

	await expect(panel).toContainText("Bot review");
	await expect(panel).not.toContainText("Please take a look");
});

test("arrow keys move between notification types", async ({ app }) => {
	const { type } = await openNotifications(app);
	await type("Humans (1)").focus();

	await app.press("ArrowRight");

	await expect(type("Agents (1)")).toBeFocused();
});

test("mark all read clears only the type on show", async ({ app }) => {
	const { type } = await openNotifications(app);
	await type("Agents (1)").click();

	await app.getByRole("button", { name: "Mark all read", exact: true }).click();

	await expect(type("Agents")).toBeVisible();
	await expect(notifications(app, 1)).toBeVisible();
});
