import type { Meta, StoryObj } from '@storybook/svelte';
import { within, userEvent } from '@storybook/testing-library';

import Page from './Page.svelte';

const meta: Meta<Page> = {
	title: 'Example/Page',
	component: Page,
	parameters: {
		// More on how to position stories at: https://storybook.js.org/docs/7.0/svelte/configure/story-layout
		layout: 'fullscreen'
	}
};

export default meta;
type Story = StoryObj<Page>;

export const LoggedOut: Story = {};

// More on interaction testing: https://storybook.js.org/docs/7.0/svelte/writing-tests/interaction-testing
export const LoggedIn: Story = {
	play: async ({ canvasElement }) => {
		const canvas = within(canvasElement);
		const loginButton = await canvas.getByRole('button', {
			name: /Log in/i
		});
		await userEvent.click(loginButton);
	}
};
