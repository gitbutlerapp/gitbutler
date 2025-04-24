import { sentrySvelteKit } from '@sentry/sveltekit';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	css: {
		preprocessorOptions: {
			scss: {
				api: 'modern-compiler'
			}
		}
	},
	plugins: [
		sentrySvelteKit({
			sourceMapsUploadOptions: {
				org: 'gitbutler',
				project: 'gitbutler-web',
				telemetry: false
			}
		}),
		sveltekit(),
		svelteTesting()
	],
	server: {
		fs: {
			strict: false
		}
	},
	test: {
		includeSource: ['src/**/*.test.{js,ts}'],
		exclude: ['node_modules/**/*', 'e2e/**/*', 'tests/**/*'],
		environment: 'jsdom',
		setupFiles: ['./vitest-setup.js']
	},
	build: {
		sourcemap: 'inline'
	}
});
