import { defineConfig } from "@playwright/test";
import path from "node:path";

const liteRoot = path.resolve(import.meta.dirname, "..");

export default defineConfig({
	testDir: "./tests",
	fullyParallel: true,
	reporter:
		process.env.CI === "true"
			? [["list"], ["buildkite-test-collector/playwright/reporter"]]
			: "list",
	webServer: {
		command: "pnpm dev:ui",
		cwd: liteRoot,
		url: "http://127.0.0.1:5173",
		reuseExistingServer: true,
		stdout: "pipe",
	},
});
