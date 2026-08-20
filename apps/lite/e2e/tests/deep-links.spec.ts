import { expect, test } from "../test.ts";

type ProbedWindow = Window & { deepLinkProbe?: boolean };

test.use({ scenario: "project-with-remote-branches.sh" });

test("a deep link navigates the open window instead of reloading it", async ({
	appWindow,
	electronApp,
}) => {
	await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace$/);
	const workspacePath = new URL(appWindow.url()).pathname;

	// Only a page load clears this, so it is what tells a navigation from a reload.
	await appWindow.evaluate(() => {
		(window as ProbedWindow).deepLinkProbe = true;
	});

	await electronApp.evaluate(({ app }, link) => {
		app.emit("open-url", { preventDefault: () => undefined }, link);
	}, `but://app${workspacePath}?page=branches`);

	await expect(appWindow).toHaveURL(/[?&]page=branches/);
	await expect(appWindow.getByText("Recent branches")).toBeVisible();
	expect(await appWindow.evaluate(() => (window as ProbedWindow).deepLinkProbe)).toBe(true);
});
