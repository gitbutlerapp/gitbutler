import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const here = path.dirname(fileURLToPath(import.meta.url));

/** The jsdom rig for the web transport; see http-transport.test.ts. */
export default defineConfig({
	root: here,
	test: {
		environment: "jsdom",
	},
});
