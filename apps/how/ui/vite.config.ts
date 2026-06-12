import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig({
	root: currentDirPath,
	plugins: [react(), tailwindcss()],
	base: "/",
	build: {
		outDir: "../dist/ui",
		emptyOutDir: true,
	},
	server: {
		port: 5174,
		strictPort: true,
	},
	resolve: {
		alias: {
			"#ui": path.join(currentDirPath, "src"),
		},
	},
});
