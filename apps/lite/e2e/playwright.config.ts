import { defineConfig } from "@playwright/test";
import path from "node:path";
import { devServerUrl } from "./setup.ts";

const liteRoot = path.resolve(import.meta.dirname, "..");

export default defineConfig({
	testDir: "./tests",
	fullyParallel: true,
	use: { viewport: { width: 1024, height: 768 } },
	reporter:
		process.env.CI === "true"
			? [["list"], ["buildkite-test-collector/playwright/reporter"]]
			: "list",
	webServer:
		devServerUrl === undefined
			? undefined
			: {
					command: "pnpm dev:ui",
					cwd: liteRoot,
					url: devServerUrl,
					reuseExistingServer: true,
					stdout: "pipe",
				},
});
