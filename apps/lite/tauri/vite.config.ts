import { defineConfig, mergeConfig } from "vite";
import { fileURLToPath } from "node:url";
import uiConfig from "../ui/vite.config.ts";

const entry = fileURLToPath(new URL("index.ts", import.meta.url));

export default defineConfig((env) =>
	mergeConfig(uiConfig(env), {
		plugins: [
			{
				name: "tauri-entry",
				transformIndexHtml: {
					order: "pre",
					handler: (html: string) => html.replace("/src/main.tsx", entry),
				},
			},
		],
		build: { outDir: "../../desktop/build/lite" },
		server: { port: 1421 },
	}),
);
