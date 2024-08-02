import { dirname, join } from 'path';
import type { StorybookConfig } from '@storybook/sveltekit';

const config: StorybookConfig = {
	stories: ['../src/stories/**/*.mdx', '../src/stories/**/*.stories.@(js|jsx|mjs|ts|tsx)'],
	addons: [
		getAbsolutePath('@storybook/addon-links'),
		getAbsolutePath('@storybook/addon-essentials'),
		getAbsolutePath('@chromatic-com/storybook'),
		getAbsolutePath('@storybook/addon-interactions'),
		getAbsolutePath('storybook-dark-mode')
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
