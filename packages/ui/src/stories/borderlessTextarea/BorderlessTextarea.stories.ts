import BorderlessTextarea from './DemoBorderlessTextarea.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / BorderlessTextarea',
	component: BorderlessTextarea
} satisfies Meta<BorderlessTextarea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const CheckboxStory: Story = {
	name: 'BorderlessTextarea',
	args: {
		name: 'BorderlessTextarea'
	},
	argTypes: {}
};
