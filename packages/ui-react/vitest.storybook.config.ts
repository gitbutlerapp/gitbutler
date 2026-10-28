import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { defineConfig } from "vitest/config";

/**
 * Every story as a test, in headless Chromium: it renders without throwing
 * and its play function passes. The stories come from .storybook/main.ts.
 */
export default defineConfig({
	plugins: [storybookTest({ configDir: ".storybook" })],
	// Base UI is pre-bundled with whichever copy of React Vite finds first; without
	// these a story renders against two Reacts and hooks read a null dispatcher.
	// Listing them up front also stops Vite re-optimizing mid-run and reloading.
	resolve: { dedupe: ["react", "react-dom"] },
	optimizeDeps: {
		include: [
			"react",
			"react/jsx-runtime",
			"react/jsx-dev-runtime",
			"react-dom",
			"react-dom/client",
			"@base-ui/react",
		],
	},
	test: {
		name: "storybook",
		browser: {
			enabled: true,
			headless: true,
			provider: "playwright",
			instances: [{ browser: "chromium" }],
		},
	},
});
