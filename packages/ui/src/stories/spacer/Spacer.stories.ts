import SpacerDemo from './SpacerDemo.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Spacer',
	component: SpacerDemo
} satisfies Meta<SpacerDemo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SpacerStory: Story = {
	name: 'Spacer',
	args: {
		margin: 12,
		noLine: false,
		dotted: false
	}
};
