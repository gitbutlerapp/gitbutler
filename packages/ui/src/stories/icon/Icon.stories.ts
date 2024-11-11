import Icon from '$lib/Icon.svelte';
import iconsJson from '$lib/data/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Icon',
	component: Icon
} satisfies Meta<Icon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'Icon',
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
