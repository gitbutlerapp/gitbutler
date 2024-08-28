import ScrollableContainer from './DemoScrollableContainer.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Scroll / ScrollableContainer',
	component: ScrollableContainer
} satisfies Meta<ScrollableContainer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Story: Story = {
	args: {
		showMode: 'hover'
	},
	argTypes: {
		showMode: {
			control: 'select',
			options: ['hover', 'always', 'onscroll']
		}
	}
};
