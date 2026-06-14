import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		environment: "jsdom",
		exclude: ["e2e/**", "node_modules/**", "dist/**"],
	},
});
