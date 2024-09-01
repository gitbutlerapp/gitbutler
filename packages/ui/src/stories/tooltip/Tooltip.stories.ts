import DemoTooltip from './DemoTooltip.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Overlays / Tooltip',
	component: DemoTooltip
} satisfies Meta<DemoTooltip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const DefaultStory: Story = {
	name: 'Tooltip',
	args: {
		position: 'bottom',
		align: 'center',
		delay: 500,
		text: 'This is a fantastic tooltip :D'
	},
	argTypes: {
		position: {
			control: 'select',
			options: ['top', 'bottom']
		},
		align: {
			control: 'select',
			options: ['start', 'center', 'end']
		}
	}
};
