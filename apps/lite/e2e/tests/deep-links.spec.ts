import { expect, test } from "../test.ts";

type ProbedWindow = Window & { deepLinkProbe?: boolean };

test.use({ scenario: "project-with-remote-branches.sh" });

test("a deep link navigates the open window instead of reloading it", async ({ app }) => {
	const workspacePath = new URL(app.page.url()).pathname;
	// Only a page load clears this, so it is what tells a navigation from a reload.
	await app.page.evaluate(() => {
		(window as ProbedWindow).deepLinkProbe = true;
	});

	await app.electronApp.evaluate(({ app }, link) => {
		app.emit("open-url", { preventDefault: () => undefined }, link);
	}, `but://app${workspacePath}?page=branches`);

	await expect(app.getByText("Recent branches")).toBeVisible();
	expect(await app.page.evaluate(() => (window as ProbedWindow).deepLinkProbe)).toBe(true);
});
