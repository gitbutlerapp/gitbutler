import { sentrySvelteKit } from '@sentry/sveltekit';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig, type Plugin } from 'vitest/config';

export default defineConfig({
	plugins: [
		debounceReload(),
		sentrySvelteKit({
			adapter: 'other',
			autoInstrument: {
				load: true,
				serverLoad: false
			},
			sourceMapsUploadOptions: {
				org: 'gitbutler',
				project: 'app-js',
				authToken: process.env.SENTRY_AUTH_TOKEN,
				telemetry: false,
				unstable_sentryVitePluginOptions: {
					telemetry: false,
					release: {
						name: process.env.SENTRY_RELEASE,
						create: true,
						setCommits: {
							auto: true,
							ignoreMissing: true,
							ignoreEmpty: true
						}
					}
				}
			}
		}),
		sveltekit(),
		svelteTesting()
	],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	// prevent vite from obscuring rust errors
	clearScreen: false,
	// tauri expects a fixed port, fail if that port is not available
	server: {
		port: 1420,
		strictPort: true,
		fs: {
			strict: false
		}
	},
	// to make use of `TAURI_ENV_DEBUG` and other env variables
	// https://tauri.studio/v1/api/config#buildconfig.beforedevcommand
	envPrefix: ['VITE_', 'TAURI_'],
	resolve: {
		conditions: ['es2015']
	},
	build: {
		// Tauri supports es2021
		target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
		// minify production builds
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		// ship sourcemaps for better sentry error reports
		sourcemap: true
	},
	test: {
		includeSource: ['src/**/*.test.{js,ts}'],
		exclude: ['node_modules/**/*', 'e2e/**/*'],
		environment: 'jsdom',
		setupFiles: ['./vitest-setup.js']
	}
});

/**
 * A module to debounce reloading when making changes to packages rather than
 * the desktop app.
 */
function debounceReload(): Plugin {
	let timeout: NodeJS.Timeout | undefined;

	return {
		name: 'debounce-reload',
		/**
		 * There is a `handleHotUpdate` callback that has the same docs, and
		 * gets called as expected, but that fails to prevent the reload.
		 */
		hotUpdate({ server, file }) {
			if (file.includes('gitbutler/packages')) {
				if (timeout) clearTimeout(timeout);
				timeout = setTimeout(() => {
					server.hot.send({ type: 'full-reload' });
					timeout = undefined;
				}, 5000);
				return []; // Prevent immediate reload.
			}
		}
	};
}
