import { sentrySvelteKit } from '@sentry/sveltekit';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [
		sentrySvelteKit({
			autoInstrument: false
		}),
		sentryVitePlugin({
			org: 'gitbutler',
			project: 'app-js',
			authToken: process.env.SENTRY_AUTH_TOKEN,
			telemetry: false,
			disable: !process.env.SENTRY_RELEASE,
			release: {
				create: true,
				cleanArtifacts: true,
				setCommits: {
					auto: true,
					ignoreMissing: true,
					ignoreEmpty: true
				}
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
	resolve: {
		conditions: ['es2015']
	},
	build: {
		// Tauri supports es2021
		target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
		// minify production builds
		minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
		// ship sourcemaps for better sentry error reports
		sourcemap: true
	},
	test: {
		deps: {
			inline: ['sorcery']
		},
		includeSource: ['src/**/*.{js,ts}'],
		exclude: ['**/e2e/playwright/**/*', 'node_modules/**/*']
	}
});
