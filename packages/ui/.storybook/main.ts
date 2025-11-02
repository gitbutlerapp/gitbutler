import { createRequire } from 'module';
import { dirname, join } from 'path';
import type { StorybookConfig } from '@storybook/sveltekit';

const require = createRequire(import.meta.url);

const config: StorybookConfig = {
	stories: ['../src/stories/**/*.mdx', '../src/stories/**/*.stories.@(js|jsx|mjs|ts|tsx|svelte)'],
	addons: [
		getAbsolutePath('@storybook/addon-docs'),
		getAbsolutePath('@storybook/addon-links'),
		getAbsolutePath('@storybook/addon-svelte-csf'),
		getAbsolutePath('@storybook/addon-vitest')
	],
	framework: {
		name: getAbsolutePath('@storybook/sveltekit'),
		options: {}
	}
};

export default config;

function getAbsolutePath(value: string): any {
	return dirname(require.resolve(join(value, 'package.json')));
}
