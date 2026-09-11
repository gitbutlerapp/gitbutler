/* oxlint-disable typescript/strict-boolean-expressions */

import posthogRollupPlugin from "@posthog/rollup-plugin";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Keeps the shell bootstrap separate from the main module, inlining deps since
 * node_modules is not shipped — except electron and @gitbutler/but-sdk,
 * whose napi loader and native modules ship as real files in the package
 * (see "files" in package.json).
 */
export default defineConfig({
	plugins: [
		{
			name: "prod-env-warning",
			apply: "build",
			buildStart() {
				if (!process.env.POSTHOG_PERSONAL_API_KEY)
					this.warn("Missing $POSTHOG_PERSONAL_API_KEY, disabling PostHog source-map uploads.");

				if (!process.env.VERSION)
					this.warn('Missing $VERSION, defaulting PostHog release version to "dev".');
			},
		},
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
	],
	define: {
		"process.env.CHANNEL": JSON.stringify(process.env.CHANNEL ?? "dev"),
	},
	build: {
		outDir: path.join(here, "../dist/electron"),
		// The preload build (vite.config.ts) runs first and empties the dir.
		emptyOutDir: false,
		target: "es2022",
		minify: false,
		sourcemap: true,
		ssr: path.join(here, "src/bootstrap.ts"),
		rollupOptions: {
			external: ["electron", /^@gitbutler\/but-sdk/],
			output: {
				entryFileNames: "main.js",
				// Main resolves the preload and UI relative to its own file.
				chunkFileNames: "[name]-[hash].js",
			},
		},
	},
	ssr: { noExternal: true },
});
