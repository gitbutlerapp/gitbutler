import type { Preview } from '@storybook/sveltekit';
import '../src/styles/main.css';
import '../src/styles/fonts/fonts.css';
import './stories-styles.css';

const preview: Preview = {
	parameters: {
		backgrounds: { disable: true },
		controls: {
			matchers: {
				// matches "background" and "color" but not "solidBackground" as that is a boolean arg
				color: /(?<!solid)(background|color)$/i,
				date: /Date$/i
			}
		},
		docs: {
			autodocs: 'tag'
		}
	},
	globalTypes: {
		theme: {
			name: 'Theme',
			description: 'Toggle between light and dark theme',
			defaultValue: 'light',
			toolbar: {
				icon: 'contrast',
				items: [
					{ value: 'light', title: 'Light mode', icon: 'sun' },
					{ value: 'dark', title: 'Dark mode', icon: 'moon' }
				],
				showName: false,
				dynamicTitle: true
			}
		}
	},
	decorators: [
		(Story, context) => {
			const theme = context.globals.theme || 'light';
			if (typeof document !== 'undefined') {
				const htmlElement = document.documentElement;

				if (theme === 'dark') {
					htmlElement.classList.add('dark');
				} else {
					htmlElement.classList.remove('dark');
				}
			}
			return Story();
		}
	]
};

export default preview;
