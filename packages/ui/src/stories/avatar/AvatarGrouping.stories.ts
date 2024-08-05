import DemoAvatarGrouping from './DemoAvatarGrouping.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoAvatarGrouping
} satisfies Meta<DemoAvatarGrouping>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AvatarGrouping: Story = {
	args: {}
};
