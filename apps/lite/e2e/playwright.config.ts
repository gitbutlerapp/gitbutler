import { defineConfig } from "@playwright/test";
import path from "node:path";

const liteRoot = path.resolve(import.meta.dirname, "..");

export default defineConfig({
	testDir: "./tests",
	fullyParallel: true,
	webServer: {
		command: "pnpm dev:ui",
		cwd: liteRoot,
		url: "http://127.0.0.1:5173",
		reuseExistingServer: true,
		stdout: "pipe",
	},
});
