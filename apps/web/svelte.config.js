import adapter from '@sveltejs/adapter-auto';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess({ script: true }),
	kit: {
		adapter: adapter(),
		alias: {
			$home: 'src/routes/(home)'
		}
	},
	compilerOptions: {
		css: 'injected',
		enableSourcemap: true
	}
};

export default config;
