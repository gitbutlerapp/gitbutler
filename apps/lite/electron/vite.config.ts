import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Bundles the preload script into one self-contained file.
 *
 * Sandboxed preloads (see `app.enableSandbox()` in main.ts) get a `require`
 * that resolves `electron` and a few polyfilled builtins — never relative
 * files. Bundling is what lets the preload import the endpoint lists from
 * ipc.ts instead of repeating every channel as a literal. Only the preload
 * needs this; the main process has its own ESM bundle (vite.main.config.ts).
 */
export default defineConfig({
	// Vite's transform covers .ts/.mts but not .cts, which the preload must
	// be so Vite emits it as CommonJS for Electron.
	esbuild: { include: /\.[cm]?[jt]sx?$/ },
	build: {
		outDir: path.join(here, "../dist/electron"),
		emptyOutDir: true,
		target: "es2022",
		minify: false,
		sourcemap: true,
		lib: {
			entry: path.join(here, "src/preload.cts"),
			formats: ["cjs"],
		},
		rollupOptions: {
			external: ["electron"],
			output: { entryFileNames: "preload.cjs" },
		},
	},
});
