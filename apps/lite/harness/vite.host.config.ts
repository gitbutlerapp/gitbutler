import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * The host binary: a node bundle the harness plugin loads, inlining all deps
 * except @gitbutler/but-sdk, whose napi loader and native module must resolve
 * as real files where the host runs.
 */
export default defineConfig({
	build: {
		outDir: path.join(here, "../dist/harness"),
		// The panel build runs first and empties the dir.
		emptyOutDir: false,
		target: "es2022",
		minify: false,
		sourcemap: true,
		ssr: path.join(here, "node/index.ts"),
		rollupOptions: {
			external: [/^@gitbutler\/but-sdk/],
			output: { entryFileNames: "node.js" },
		},
	},
	ssr: { noExternal: true },
});
