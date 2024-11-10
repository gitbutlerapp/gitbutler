import allIcons from './AllIcons.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Icon / All icons',
	component: allIcons
} satisfies Meta<allIcons>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'All icons'
};
