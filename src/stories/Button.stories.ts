import type { Meta, StoryObj } from '@storybook/svelte';

import Button from '$lib/components/Button.svelte';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/svelte/writing-stories/introduction
const meta: Meta<Button> = {
	title: 'GitButler/Button',
	component: Button,
	tags: ['autodocs'],
	argTypes: {
		primary: { control: 'boolean' },
		outlined: { control: 'boolean' },
		small: { control: 'boolean' },
		wide: { control: 'boolean' },
		label: { control: 'text' }
	}
};

export default meta;
type Story = StoryObj<Button>;

// More on writing stories with args: https://storybook.js.org/docs/7.0/svelte/writing-stories/args
export const Basic: Story = {
	args: {
		primary: false,
		outlined: false,
		label: 'Label'
	}
};

export const BasicOutlined: Story = {
	args: {
		primary: false,
		outlined: true,
		label: 'Button'
	}
};

export const BasicSmall: Story = {
	args: {
		primary: false,
		outlined: false,
		small: true,
		label: 'Button'
	}
};

export const BasicOutlinedSmall: Story = {
	args: {
		primary: false,
		outlined: true,
		small: true,
		label: 'Button'
	}
};


export const Primary: Story = {
	args: {
		primary: true,
		outlined: false,
		label: 'Label'
	}
};
export const PrimaryOutlined: Story = {
	args: {
		primary: true,
		outlined: true,
		label: 'Label'
	}
};
export const PrimarySmall: Story = {
	args: {
		primary: true,
		outlined: false,
		small: true,
		label: 'Label'
	}
};

export const PrimaryWide: Story = {
	args: {
		primary: true,
		outlined: false,
		wide: true,
		label: 'Label'
	}
};

export const PrimarySmallWide: Story = {
	args: {
		primary: true,
		outlined: false,
		wide: true,
		small: true,
		label: 'Label'
	}
};

export const PrimaryOutlinedSmall: Story = {
	args: {
		primary: true,
		outlined: true,
		small: true,
		label: 'Label'
	}
};


