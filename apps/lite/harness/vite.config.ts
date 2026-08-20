import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * The panel binary: a single self-contained IIFE the harness client can
 * evaluate and call, React 19 bundled in, CSS as one file alongside.
 *
 * `window.lite.platform` is a compile-time define because hotkeys.ts reads it
 * at module scope, before the entry can install the api. The panel runs on
 * the machine it was built on, so the build-time platform is correct.
 */
export default defineConfig({
	root: here,
	plugins: [
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	define: {
		"process.env.NODE_ENV": JSON.stringify("production"),
		"window.lite.platform": JSON.stringify(process.platform),
		"globalThis.window.lite.platform": JSON.stringify(process.platform),
	},
	build: {
		outDir: path.join(here, "../dist/harness"),
		emptyOutDir: true,
		target: "es2022",
		cssCodeSplit: false,
		lib: {
			entry: path.join(here, "browser/index.tsx"),
			name: "createButPanel",
			formats: ["iife"],
			fileName: () => "browser.js",
		},
	},
	worker: {
		format: "es",
		rollupOptions: { output: { inlineDynamicImports: true } },
	},
});
