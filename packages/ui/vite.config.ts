import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
	plugins: [sveltekit()],
	resolve: {
		alias: {
			$components: path.resolve('./src/lib/components')
		}
	},
	test: {
		include: ['src/**/*.(test|spec).?(m)[jt]s?(x)']
	},
	build: {
		sourcemap: 'inline'
	}
});
