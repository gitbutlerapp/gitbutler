import SegmentedControl from './SegmentedControl.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: SegmentedControl
} satisfies Meta<SegmentedControl>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SegmentedControlStory: Story = {
	args: {
		selectedIndex: 1
	}
};
