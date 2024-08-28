import DemoScrollbar from './DemoScrollbar.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Scroll / Scrollbar',
	component: DemoScrollbar
} satisfies Meta<DemoScrollbar>;

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
