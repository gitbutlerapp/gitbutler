import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [sveltekit()],
	test: {
		include: ['src/**/*.(test|spec).?(m)[jt]s?(x)'],
		environment: 'jsdom'
	},
	build: {
		sourcemap: 'inline'
	}
});
