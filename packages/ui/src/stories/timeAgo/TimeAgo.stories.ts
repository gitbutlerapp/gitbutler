import DemoTimeAgo from './DemoTimeAgo.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Time Ago',
	component: DemoTimeAgo,
	argTypes: {
		date: {
			control: 'date'
		}
	}
} satisfies Meta<DemoTimeAgo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const TimeAgoSuffixless: Story = {
	args: {
		date: 1721315627068,
		addSuffix: false
	}
};

export const TimeAgoWithSuffix: Story = {
	args: {
		date: 1721315627068,
		addSuffix: true
	}
};
