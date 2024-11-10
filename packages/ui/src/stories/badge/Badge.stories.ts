import DemoBadge from './DemoBadge.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Badge',
	component: DemoBadge
} satisfies Meta<DemoBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const BadgeStory: Story = {
	name: 'Badge',
	args: {
		label: '127',
		style: 'neutral',
		kind: 'solid'
	},
	argTypes: {
		style: {
			options: ['neutral', 'success', 'warning', 'error'],
			control: { type: 'select' }
		},
		kind: {
			options: ['solid', 'soft'],
			control: { type: 'select' }
		}
	}
};
