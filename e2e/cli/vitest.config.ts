import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		testTimeout: 120_000,
		hookTimeout: 60_000,
		// Run test files sequentially to avoid rate-limit and repo contention issues.
		fileParallelism: false,
		// Tests within a file run sequentially (they share repo state).
		sequence: { concurrent: false },
	},
});
