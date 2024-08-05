import DemoModal from './DemoModal.svelte';
import iconsJson from '$lib/icon/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoModal,
	argTypes: {
		width: {
			control: 'select',
			options: ['default', 'small', 'large']
		},
		title: { control: 'text' },
		icon: {
			control: 'select',
			options: [undefined, ...Object.keys(iconsJson)]
		}
	}
} satisfies Meta<DemoModal>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ModalStory: Story = {
	args: {
		width: 'small',
		title: 'This is a fantastic modal :D'
	}
};
