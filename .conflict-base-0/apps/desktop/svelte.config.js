import staticAdapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess({ script: true }),
	kit: {
		alias: {
			$components: './src/components'
		},
		adapter: staticAdapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: true,
			strict: false
		})
	},
	compilerOptions: {
		css: 'injected',
		enableSourcemap: true
	}
};

export default config;
