import { defineConfig } from "@playwright/test";

export default defineConfig({
	testDir: "./tests",
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	workers: 1,
	timeout: 60_000,
	expect: { timeout: 10_000 },
	reporter: process.env.CI ? "github" : "dot",
	use: {
		trace: "retain-on-failure",
	},
});
