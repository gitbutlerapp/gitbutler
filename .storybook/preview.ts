import type { Preview } from '@storybook/svelte';
import '../src/app.postcss';
const preview: Preview = {
	parameters: {
		actions: { argTypesRegex: '^on[A-Z].*' },
		controls: {
			matchers: {
				color: /(background|color)$/i,
				date: /Date$/
			}
		},
		backgrounds: {
			default: 'GitButler_1',
			values: [
				{
					name: 'GitButler_1',
					value: '#27272A'
				}
			]
		}
	}
};

export default preview;
