import type { Meta, StoryObj } from '@storybook/svelte';

import TooltipOnText from './TooltipOnText.svelte';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/svelte/writing-stories/introduction
const meta: Meta<TooltipOnText> = {
	title: 'GitButler/Tooltip',
	component: TooltipOnText,
	tags: ['autodocs'],
	argTypes: {
		label: { control: 'text' }
	}
};

export default meta;
type Story = StoryObj<TooltipOnText>;

export const TextWithTooltip: Story = {
	args: {
		label: 'This is a tooltip'
	}
};
