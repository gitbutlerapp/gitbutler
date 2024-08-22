import DemoBadge from './DemoBadge.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Elements / Badge',
	component: DemoBadge
} satisfies Meta<DemoBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'Badge',
	args: {
		label: '127',
		help: 'This is a badge'
	}
};
