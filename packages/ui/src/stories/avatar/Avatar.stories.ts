import Avatar from '$lib/avatar/Avatar.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: Avatar,

	argTypes: {
		size: {
			control: 'select',
			options: ['small', 'medium']
		}
	}
} satisfies Meta<Avatar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AvatarSmall: Story = {
	args: {
		srcUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c',
		tooltip: 'The avatar of bob',
		size: 'small'
	}
};

export const AvatarMedium: Story = {
	args: {
		srcUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c',
		tooltip: 'The avatar of bob',
		size: 'medium'
	}
};
