import svelteInjectComment from "@gitbutler/svelte-comment-injector";
import staticAdapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// vitePreprocess({ script: true }) uses transformWithOxc, which strips imports
// that appear unused in a single script block — this breaks the Svelte pattern
// where <script module> uses a value imported in <script>. Skip TS preprocessing
// for node_modules files to avoid removing cross-block imports like createCommand.
const vitePreprocessors = vitePreprocess({ script: true });
const filteredVitePreprocess = {
	...vitePreprocessors,
	script: vitePreprocessors.script
		? async (args) => {
				if (args.filename?.includes("node_modules")) return;
				return await vitePreprocessors.script(args);
			}
		: undefined,
	// Svelte 5's CSS parser rejects `@import "url" (condition)` inside `:global {}` blocks
	// (e.g. svelte-lexical's BlueskyPostComponent.svelte). Strip all @import rules from
	// node_modules style blocks — they reference external URLs that won't resolve in the
	// bundle anyway, so removing them is safe.
	style: async (args) => {
		if (!args.filename?.includes("node_modules")) return;
		const code = args.content.replace(/@import\s+[^;]+;/g, "");
		return code !== args.content ? { code } : undefined;
	},
};

const config = {
	preprocess: [filteredVitePreprocess, svelteInjectComment()],
	kit: {
		alias: {
			$components: "./src/components",
		},
		adapter: staticAdapter({
			pages: "build",
			assets: "build",
			fallback: "index.html",
			precompress: false,
			strict: false,
		}),
	},
	compilerOptions: {
		css: "external",
	},
	vitePlugin: {
		dynamicCompileOptions({ filename }) {
			if (filename.includes("node_modules")) {
				return { css: "injected" };
			}
		},
	},
};

export default config;
