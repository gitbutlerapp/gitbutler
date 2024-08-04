import { sentrySvelteKit } from '@sentry/sveltekit';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sentrySvelteKit({
			sourceMapsUploadOptions: {
				org: 'gitbutler',
				project: 'gitbutler-web',
				telemetry: false
			}
		}),
		sveltekit()
	],
	server: {
		fs: {
			strict: false
		}
	}
});
