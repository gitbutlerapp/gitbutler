import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

/** The app's renderer as a web page, with `window.lite` over HTTP to a but-server. */
export default defineConfig(({ command }) => ({
	root: here,
	define: {
		"process.env.CHANNEL": JSON.stringify(process.env.CHANNEL ?? "dev"),
	},
	plugins: [
		react({
			babel: {
				plugins: ["babel-plugin-react-compiler"],
			},
		}),
	],
	build: {
		outDir: path.join(here, "../dist/web"),
		emptyOutDir: true,
		target: "es2022",
	},
	worker: {
		format: "es",
	},
	server: {
		port: 5174,
		strictPort: true,
	},
	...(command === "serve" && {
		css: {
			modules: {
				generateScopedName: "[name]_[local]__[hash:base64:5]",
			},
		},
	}),
}));
