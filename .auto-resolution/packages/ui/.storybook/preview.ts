import type { Preview } from '@storybook/svelte';
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
		darkMode: {
			classTarget: 'html',
			stylePreview: true,
			dark: {
				appPreviewBg: '#272321'
			},
			light: {
				appPreviewBg: '#fff'
			}
		}
	}
};

export default preview;
