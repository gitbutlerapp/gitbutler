import { defineConfig, mergeConfig } from "vite";
import uiConfig from "../ui/vite.config.ts";

export default defineConfig((env) =>
	mergeConfig(uiConfig(env), {
		plugins: [
			{
				name: "tauri-entry",
				transformIndexHtml: {
					order: "pre",
					handler: (html: string) => html.replace("/src/main.tsx", "../tauri/index.ts"),
				},
			},
		],
		build: { outDir: "../../desktop/build/lite" },
	}),
);
