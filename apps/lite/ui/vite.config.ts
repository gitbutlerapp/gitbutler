import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig(({ command }) => ({
	root: currentDirPath,
	plugins: [
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	base: "/",
	build: {
		outDir: "../dist/ui",
		emptyOutDir: true,
	},
	worker: {
		format: "es",
	},
	server: {
		port: 5173,
		strictPort: true,
	},
	// Improve readability of class names in development mode by adding the module
	// name as a prefix, e.g. `MyComponent_myClass__abc123`.
	...(command === "serve" && {
		css: {
			modules: {
				generateScopedName: "[name]_[local]__[hash:base64:5]",
			},
		},
	}),
}));
