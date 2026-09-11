/* oxlint-disable typescript/strict-boolean-expressions */

import posthogRollupPlugin from "@posthog/rollup-plugin";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig(({ command }) => ({
	root: currentDirPath,
	define: {
		"process.env.CHANNEL": JSON.stringify(process.env.CHANNEL ?? "dev"),
	},
	plugins: [
		{
			name: "posthog-sourcemap-warning",
			apply: "build",
			buildStart() {
				if (!process.env.POSTHOG_PERSONAL_API_KEY)
					this.warn("Missing $POSTHOG_PERSONAL_API_KEY, PostHog sourcemap uploads disabled.");

				if (!process.env.VERSION)
					this.warn('Missing $VERSION, PostHog release version defaults to "dev".');
			},
		},
		command === "build" &&
			!!process.env.POSTHOG_PERSONAL_API_KEY &&
			posthogRollupPlugin({
				personalApiKey: process.env.POSTHOG_PERSONAL_API_KEY,
				projectId: "2812",
				host: "https://eu.posthog.com",
				sourcemaps: {
					releaseName: "gitbutler-next",
					releaseVersion: process.env.VERSION || "dev",
					deleteAfterUpload: true,
				},
			}),
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	base: "/",
	build: {
		outDir: "../dist/ui",
		emptyOutDir: true,
	},
	worker: {
		format: "es",
	},
	server: {
		port: 5173,
		strictPort: true,
	},
	// Improve readability of class names in development mode by adding the module
	// name as a prefix, e.g. `MyComponent_myClass__abc123`.
	...(command === "serve" && {
		css: {
			modules: {
				generateScopedName: "[name]_[local]__[hash:base64:5]",
			},
		},
	}),
	// Keep at least the property here to help Knip's inference.
	test: {},
}));
