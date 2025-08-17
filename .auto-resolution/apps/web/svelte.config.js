import adapter from '@sveltejs/adapter-vercel';
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
		css: 'injected'
	}
};

export default config;
