import staticAdapter from '@sveltejs/adapter-static';
import svelteInjectComment from '@gitbutler/svelte-comment-injector';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: [vitePreprocess({ script: true }), svelteInjectComment()],
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
		css: 'external'
	}
};

asdfasdff
export default config;
