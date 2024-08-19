import SegmentControl from './DemoSegmentControl.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Segment Control',
	component: SegmentControl
} satisfies Meta<SegmentControl>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SegmentControlStory: Story = {
	name: 'Segment Control',
	args: {
		defaultIndex: 1,
		fullWidth: false,
		segments: [
			{ label: 'First', id: 'first' },
			{ label: 'Second', id: 'second' },
			{ label: 'Third', id: 'third' },
			{ label: 'Fourth', id: 'fourth' }
		]
	}
};
