import type { Preview } from '@storybook/svelte';
import '../src/styles/main.css';
import '../src/stories/stories-styles.css';

const preview: Preview = {
	parameters: {
		controls: {
			matchers: {
				color: /(background|color)$/i,
				date: /Date$/i
			}
		}
	}
};

export default preview;
