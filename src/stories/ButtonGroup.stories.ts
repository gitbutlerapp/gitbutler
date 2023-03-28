import type { Meta, StoryObj } from '@storybook/svelte';

import ButtonGroup from '$lib/components/ButtonGroup.svelte';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/svelte/writing-stories/introduction
const meta: Meta<ButtonGroup> = {
	title: 'GitButler/ButtonGroup',
	component: ButtonGroup,
	tags: ['autodocs'],
	argTypes: {
		leftLabel: { control: 'text' },
		rightLabel: { control: 'text' },
		middleLabel: { control: 'text' }
	}
};

export default meta;
type Story = StoryObj<ButtonGroup>;

export const TwoButtons: Story = {
	args: {
		leftLabel: 'Cancel',
		rightLabel: 'Submit'
	}
};

export const TwoButtonsWide: Story = {
	args: {
		leftLabel: 'Cancel',
		rightLabel: 'Submit',
		wide: true
	}
};

export const ThreeButtons: Story = {
	args: {
		leftLabel: 'Left',
		middleLabel: 'Middle',
		rightLabel: 'Right'
	}
};

export const ThreeButtonsWide: Story = {
	args: {
		leftLabel: 'Left',
		middleLabel: 'Middle',
		rightLabel: 'Right',
		wide: true
	}
};
