import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { sentrySvelteKit } from '@sentry/sveltekit';

export default defineConfig({
	plugins: [
		sentrySvelteKit({
			sourceMapsUploadOptions: {
				dryRun: process.env.SENTRY_RELEASE === undefined,
				org: 'gitbutler',
				project: 'desktop',
				// this is nikita galaiko's personal sentry api token.
				authToken: '04c6bc1df15346f39ed2fbeb99c0a8e25bcbedc4aba9461bb3a471733b8c80db',
				include: ['build'],
				cleanArtifacts: true,
				setCommits: {
					auto: true,
					ignoreMissing: true,
					ignoreEmpty: true
				},
				telemetry: false,
				uploadSourceMaps: process.env.SENTRY_RELEASE !== undefined
			}
		}),
		sveltekit()
	],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	// prevent vite from obscuring rust errors
	clearScreen: false,
	// tauri expects a fixed port, fail if that port is not available
	server: {
		port: 1420,
		strictPort: true
	},
	// to make use of `TAURI_DEBUG` and other env variables
	// https://tauri.studio/v1/api/config#buildconfig.beforedevcommand
	envPrefix: ['VITE_', 'TAURI_'],
	build: {
		// Tauri supports es2021
		target: process.env.TAURI_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
		// minify production builds
		minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
		// ship sourcemaps for better sentry error reports
		sourcemap: 'inline'
	}
});
