import { defineWorkspace } from 'vitest/config';
import { storybookTest } from '@storybook/experimental-addon-vitest/plugin';
import { storybookSveltekitPlugin } from '@storybook/sveltekit/vite'

export default defineWorkspace([
  'vite.config.ts',
  {
    extends: 'vite.config.ts',
    plugins: [
      storybookTest({ storybookScript: 'pnpm storybook' }),
      storybookSveltekitPlugin()
    ],
    test: {
      browser: {
        enabled: true,
        headless: true,
        name: 'chromium',
        provider: 'playwright',
      },
      include: ['**/*.stories.?(m)[jt]s?(x)'],
      setupFiles: ['./.storybook/vitest.setup.ts'],
    },
  },
]);
