import Textarea from './Textarea.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Textarea',
	component: Textarea
} satisfies Meta<Textarea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const CheckboxStory: Story = {
	name: 'Textarea',
	args: {
		value: 'Hello, World!',
		placeholder: 'Type here...',
		minRows: 1,
		maxRows: 5,
		autofocus: false,
		disabled: false,
		borderTop: true,
		borderRight: true,
		borderBottom: true,
		borderLeft: true,
		unstyled: false
	}
};
