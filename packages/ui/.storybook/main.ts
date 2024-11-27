import { dirname, join } from 'path';
import type { StorybookConfig } from '@storybook/sveltekit';

const config: StorybookConfig = {
	stories: ['../src/stories/**/*.mdx', '../src/stories/**/*.stories.@(js|jsx|mjs|ts|tsx|svelte)'],

	addons: [
		getAbsolutePath('@storybook/addon-links'),
		getAbsolutePath('@storybook/addon-essentials'),
		getAbsolutePath('@storybook/experimental-addon-test'),
		getAbsolutePath('storybook-dark-mode')
	],

	framework: {
		name: getAbsolutePath('@storybook/sveltekit'),
		options: {}
	},

	docs: {}
};
export default config;

function getAbsolutePath(value: string): any {
	return dirname(require.resolve(join(value, 'package.json')));
}
