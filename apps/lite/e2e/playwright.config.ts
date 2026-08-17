import { defineConfig } from "@playwright/test";
import path from "node:path";

const liteRoot = path.resolve(import.meta.dirname, "..");

export default defineConfig({
	testDir: "./tests",
	fullyParallel: true,
	// Empties the capture directory once per run when SCREENSHOT_OUT is set, and
	// does nothing otherwise. It cannot live in the spec's beforeAll: that hook
	// runs again on every worker restart, so a single failing surface would delete
	// the ones already captured.
	globalSetup: "./clear-screenshots.ts",
	webServer: {
		command: "pnpm dev:ui",
		cwd: liteRoot,
		url: "http://127.0.0.1:5173",
		reuseExistingServer: true,
		stdout: "pipe",
	},
});
