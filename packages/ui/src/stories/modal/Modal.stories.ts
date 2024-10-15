import DemoModal from './DemoModal.svelte';
import type { StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Overlays / Modal',
	component: DemoModal as any,
	argTypes: {
		width: {
			control: 'select',
			options: ['default', 'small', 'large']
		},
		title: { control: 'text' },
		type: {
			control: 'select',
			options: ['info', 'success', 'warning', 'error']
		}
	}
};

export default meta;
type Story = StoryObj<typeof meta>;

export const DefaultStory: Story = {
	name: 'Modal',
	args: {
		width: 'small',
		type: 'info',
		title: 'This is a fantastic modal :D'
	}
};
