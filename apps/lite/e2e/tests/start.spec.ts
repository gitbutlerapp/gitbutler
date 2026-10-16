import { expect, test } from "../test.ts";

test("starts on onboarding without configured projects", async ({ app }) => {
	await expect(app.page).toHaveTitle("GitButler Next Nightly");
	await expect(app.onboardingHeading).toBeVisible();
});

test.describe("with a seeded project", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("opens the project's workspace", async ({ app }) => {
		await expect(app.page).toHaveURL(/\/project\/[^/]+\/workspace$/);
		await expect(app.projectPicker).toBeVisible();
	});

	test("the branches view takes input", async ({ app }) => {
		await app.goTo("Branches");

		// The filter is folded into the header until asked for, so opening it is
		// what proves the tab is interactive rather than merely painted.
		await app.getByRole("button", { name: "Filter branches" }).click();
		await expect(app.getByRole("textbox", { name: "Filter branches" })).toBeVisible();
	});
});
