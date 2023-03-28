import type { Meta, StoryObj } from '@storybook/svelte';

import Button from '$lib/components/Button.svelte';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/svelte/writing-stories/introduction
const meta: Meta<Button> = {
	title: 'GitButler/Button',
	component: Button,
	tags: ['autodocs'],
	argTypes: {
		primary: { control: 'boolean' },
		filled: { control: 'boolean' },
		small: { control: 'boolean' },
		label: { control: 'text' }
	}
};

export default meta;
type Story = StoryObj<Button>;

// More on writing stories with args: https://storybook.js.org/docs/7.0/svelte/writing-stories/args
export const Primary: Story = {
	args: {
		primary: true,
		filled: false,
		label: 'Button'
	}
};
export const PrimaryFilled: Story = {
	args: {
		primary: true,
		filled: true,
		label: 'Button'
	}
};
export const PrimarySmall: Story = {
	args: {
		primary: true,
		filled: false,
		small: true,
		label: 'Button'
	}
};

export const PrimaryWide: Story = {
	args: {
		primary: true,
		filled: false,
		wide: true,
		label: 'Button'
	}
};

export const PrimarySmallWide: Story = {
	args: {
		primary: true,
		filled: false,
		wide: true,
		small: true,
		label: 'Button'
	}
};


export const PrimaryFilledSmall: Story = {
	args: {
		primary: true,
		filled: true,
		small: true,
		label: 'Button'
	}
};

export const Default: Story = {
	args: {
		primary: false,
		filled: false,
		label: 'Button'
	}
};

export const DefaultFilled: Story = {
	args: {
		primary: false,
		filled: true,
		label: 'Button'
	}
};

export const DefaultSmall: Story = {
	args: {
		primary: false,
		filled: false,
		small: true,
		label: 'Button'
	}
};

export const DefaultFilledSmall: Story = {
	args: {
		primary: false,
		filled: true,
		small: true,
		label: 'Button'
	}
};
