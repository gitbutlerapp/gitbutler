import DemoInfoButton from './DemoInfoButton.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Overlays / InfoButton',
	component: DemoInfoButton
} as Meta<DemoInfoButton>;

export default meta;

type Story = StoryObj<typeof meta> & {
	args: {
		title: string | undefined;
		size: 'small' | 'medium';
	};
	argTypes: any;
};

export const DefaultStory: Story = {
	name: 'InfoButton',
	args: {
		title: 'Tooltip title',
		size: 'medium'
	},
	argTypes: {
		size: {
			options: ['small', 'medium'],
			control: { type: 'select' }
		}
	}
};
