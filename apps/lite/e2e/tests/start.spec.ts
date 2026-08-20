import { LiteTestId } from "../../ui/src/testIds.ts";
import { expect, test } from "../test.ts";

test("starts without configured projects", async ({ appWindow }) => {
	await expect(appWindow).toHaveTitle("GitButler Lite");
	await expect(appWindow.getByTestId(LiteTestId.OnboardingPage)).toBeVisible();
});

test.describe("with a seeded project", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("opens the project and navigates between views", async ({ appWindow }) => {
		await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace$/);
		await expect(appWindow.getByRole("button", { name: /Select project/ })).toBeVisible();

		const pages = appWindow.getByRole("group", { name: "Pages" });
		await pages.getByRole("button", { name: "Branches" }).click();
		// The header, not the list: the seeded remote branches are filtered out by
		// the default local-only filter, so the tree is empty and has no height.
		await expect(appWindow.getByText("Recent branches")).toBeVisible();

		// The filter is folded into the header until asked for, so opening it is
		// what proves the tab is interactive rather than merely painted.
		await appWindow.getByRole("button", { name: "Filter branches" }).click();
		await expect(appWindow.getByRole("textbox", { name: "Filter branches" })).toBeVisible();
	});
});
