import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDirectory = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig(({ mode }) => {
	const input = mode === "review" ? "review.html" : "workspace.html";

	return {
		root: currentDirectory,
		plugins: [react(), viteSingleFile()],
		build: {
			outDir: path.resolve(currentDirectory, "../../crates/but/src/command/mcp"),
			emptyOutDir: false,
			rollupOptions: {
				input: path.resolve(currentDirectory, input),
			},
		},
	};
});
