import { expect, test } from "../test.ts";

test("starts without configured projects", async ({ appWindow }) => {
	await expect(appWindow).toHaveTitle("GitButler Lite");
	await expect(appWindow.getByRole("main")).toContainText("Select a project.");
});

test.describe("with a controlled login shell", () => {
	test.use({ useLoginShell: true });

	test("imports its environment before startup continues", async ({ electronApp }) => {
		const imported = await electronApp.evaluate(() => process.env.GITBUTLER_LOGIN_SHELL_IMPORTED);
		expect(imported).toBe("1");
	});
});

test.describe("without an available login shell", () => {
	test.use({
		environmentVariables: {
			SHELL: "/missing/gitbutler-login-shell",
			TERM: undefined,
		},
	});

	test("continues startup with the GUI launcher environment", async ({ appWindow }) => {
		await expect(appWindow).toHaveTitle("GitButler Lite");
		await expect(appWindow.getByRole("main")).toContainText("Select a project.");
	});
});

test.describe("with a seeded project", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("opens the project and navigates between views", async ({ appWindow }) => {
		await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace$/);
		await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();

		const navigation = appWindow.getByRole("group", { name: "Navigation" });
		await navigation.getByRole("button", { name: "Branches" }).click();
		await expect(appWindow.getByRole("textbox", { name: "Filter branches" })).toBeVisible();
	});
});
