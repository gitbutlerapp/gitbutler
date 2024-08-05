import DemoAllButtons from './DemoAllButtons.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Button / All Buttons',
	component: DemoAllButtons
} satisfies Meta<DemoAllButtons>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ButtonClickable: Story = {
	name: 'All Buttons',
	args: {
		label: 'Button',
		reversedDirection: false
	}
};
