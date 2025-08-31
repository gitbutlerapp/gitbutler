import svelteInjectComment from '@gitbutler/svelte-comment-injector';
import staticAdapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

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

export default config;
