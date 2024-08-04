import SegmentControl from './SegmentControl.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Controls / Segment Control',
	component: SegmentControl
} satisfies Meta<SegmentControl>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SegmentControlStory: Story = {
	name: 'Segment Control',
	args: {
		defaultIndex: 1,
		fullWidth: false
	}
};
