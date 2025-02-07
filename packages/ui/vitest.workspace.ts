import { storybookTest } from '@storybook/experimental-addon-test/vitest-plugin';
import { storybookSveltekitPlugin } from '@storybook/sveltekit/vite-plugin';
import { defineWorkspace } from 'vitest/config';

export default defineWorkspace([
	'vite.config.ts',
	{
		extends: 'vite.config.ts',
		plugins: [storybookTest({ storybookScript: 'pnpm storybook' }), storybookSveltekitPlugin()],
		test: {
			browser: {
				enabled: true,
				headless: true,
				name: 'chromium',
				provider: 'playwright'
			},
			setupFiles: ['./.storybook/vitest.setup.ts']
		}
	}
]);
