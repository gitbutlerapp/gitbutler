import react from "@vitejs/plugin-react";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig({
	root: currentDirPath,
	plugins: [
		tanstackRouter({
			target: "react",
			quoteStyle: "double",
		}),
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	build: {
		outDir: "../dist/ui",
		emptyOutDir: true,
	},
	server: {
		port: 5173,
		strictPort: true,
	},
});
