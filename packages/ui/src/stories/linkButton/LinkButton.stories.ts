import DemoLinkButton from './DemoLinkButton.svelte';
import iconsJson from '$lib/data/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Link Button',
	component: DemoLinkButton
} satisfies Meta<DemoLinkButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'Link Button',
	args: {
		icon: 'copy-small',
		onclick: () => {
			console.log('Button clicked');
		}
	},
	argTypes: {
		icon: {
			control: 'select',
			options: Object.keys(iconsJson)
		}
	}
};
