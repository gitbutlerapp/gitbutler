import DemoToggle from './DemoToggle.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Toggle',
	component: DemoToggle as any
} satisfies Meta<typeof DemoToggle>;

export default meta;
type Story = StoryObj<typeof meta>;

export const DefaultStory: Story = {
	name: 'Toggle',
	args: {
		name: 'Toggle',
		checked: false,
		disabled: false,
		small: false
	}
};
