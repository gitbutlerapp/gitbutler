import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const here = path.dirname(fileURLToPath(import.meta.url));

/** The jsdom rig for the panel bundle; see tests/panel.test.tsx. */
export default defineConfig({
	root: here,
	plugins: [
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	worker: {
		format: "es",
		rollupOptions: { output: { inlineDynamicImports: true } },
	},
	test: {
		environment: "jsdom",
		setupFiles: [path.join(here, "tests/setup.ts")],
	},
});
