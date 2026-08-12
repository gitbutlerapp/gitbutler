import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Bundles the main process into one ESM file, inlining all deps since
 * node_modules is not shipped — except electron and @gitbutler/but-sdk,
 * whose napi loader and native modules ship as real files in the package
 * (see "files" in package.json).
 */
export default defineConfig({
	build: {
		outDir: path.join(here, "../dist/electron"),
		// The preload build (vite.config.ts) runs first and empties the dir.
		emptyOutDir: false,
		target: "es2022",
		minify: false,
		sourcemap: true,
		ssr: path.join(here, "src/main.ts"),
		rollupOptions: {
			external: ["electron", /^@gitbutler\/but-sdk/],
			output: { entryFileNames: "main.js" },
		},
	},
	ssr: { noExternal: true },
});
