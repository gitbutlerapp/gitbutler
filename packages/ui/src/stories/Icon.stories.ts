import Icon from '$lib/icon/Icon.svelte';
import iconsJson from '$lib/icon/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: Icon
} satisfies Meta<Icon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	args: {
		name: 'ai',
		color: 'pop'
	},
	argTypes: {
		color: {
			control: 'select',
			options: ['neutral', 'ghost', 'pop', 'success', 'error', 'warning', 'purple', undefined]
		},
		name: {
			control: 'select',
			options: Object.keys(iconsJson)
		}
	}
};
