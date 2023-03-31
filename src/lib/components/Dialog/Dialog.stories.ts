import type { Meta, StoryObj } from '@storybook/svelte';

import DialogWithButton from './DialogWithButton.svelte';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/svelte/writing-stories/introduction
const meta: Meta<DialogWithButton> = {
	title: 'GitButler/Dialog',
	component: DialogWithButton,
	tags: ['autodocs'],
	argTypes: {
		title: { control: 'text' },
		content: { control: 'text' },
		secondaryActionLabel: { control: 'text' },
		primaryActionLabel: { control: 'text' },
		size: { control: 'text' }
	}
};

export default meta;
type Story = StoryObj<DialogWithButton>;

export const DialogWithTitleOnly: Story = {
	args: {
		title: 'Dialog Title'
	}
};

export const DialogWithTitleAndBody: Story = {
	args: {
		title: 'Dialog Title',
		content: 'Dialog body content'
	}
};

export const DialogSmall: Story = {
	args: {
		title: 'Dialog Title',
		content: 'Dialog body content',
		size: 'small'
	}
};

export const DialogLarge: Story = {
	args: {
		title: 'Dialog Title',
		content: 'Dialog body content',
		size: 'large'
	}
};
