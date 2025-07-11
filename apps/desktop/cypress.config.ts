import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'cypress';
import path from 'path';

export default defineConfig({
	retries: {
		// Configure retry attempts for `cypress run`
		runMode: 2,
		// Configure retry attempts for `cypress open`
		openMode: 0
	},

	e2e: {
		baseUrl: 'http://localhost:1420',
		supportFile: 'cypress/e2e/support/index.ts'
	},

	experimentalWebKitSupport: true,

	component: {
		devServer: {
			framework: 'svelte',
			bundler: 'vite',
			viteConfig: {
				plugins: [svelte()],
				resolve: {
					alias: {
						$components: path.resolve('src/components'),
						$lib: path.resolve('src/lib')
					}
				}
			}
		},
		// 👇 And this line if Cypress still fails to resolve the iframe mount file
		indexHtmlFile: 'cypress/support/index.html'
	}
});
