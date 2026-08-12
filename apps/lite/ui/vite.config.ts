import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ipcTraceHost, ipcTracePlugin, ipcTraceWatcherHost } from "../electron/src/tracing.js";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig(({ command }) => ({
	root: currentDirPath,
	plugins: [
		ipcTracePlugin(),
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
		allowedHosts: [ipcTraceHost, ipcTraceWatcherHost],
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
	// Keep at least the property here to help Knip's inference.
	test: {},
}));
