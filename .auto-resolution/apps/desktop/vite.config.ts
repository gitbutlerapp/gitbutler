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
	build: {
		rollupOptions: { output: { manualChunks: {} } },
		// Tauri supports es2021
		target: 'modules',
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
	let mustReload = false;
	let longDelay = false;

	return {
		name: 'debounce-reload',
		/**
		 * There is a `handleHotUpdate` callback that has the same docs, and
		 * gets called as expected, but that fails to prevent the reload.
		 */
		hotUpdate({ server, file }) {
			if (!file.includes('apps/desktop')) {
				mustReload = true;
				longDelay = true;
			} else if (file.includes('.svelte-kit')) {
				mustReload = true;
			}
			if (mustReload) {
				clearTimeout(timeout);
				timeout = setTimeout(
					() => {
						timeout = undefined;
						mustReload = false;
						longDelay = false;
						server.hot.send({ type: 'full-reload' });
					},
					longDelay ? 5000 : 250
				);
				server.hot.send('gb:reload');
				return []; // Prevent immediate reload.
			}
		}
	};
}
