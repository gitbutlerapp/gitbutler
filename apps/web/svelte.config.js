import nodeAdapter from "@sveltejs/adapter-node";
import vercelAdapter from "@sveltejs/adapter-vercel";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess({ script: true }),
	kit: {
		adapter: process.env.BUILD_TARGET === "node" ? nodeAdapter() : vercelAdapter(),
		alias: {
			$home: "src/routes/(home)",
			$scripts: "../../scripts",
		},
	},
	compilerOptions: {
		css: "injected",
	},
};

export default config;
