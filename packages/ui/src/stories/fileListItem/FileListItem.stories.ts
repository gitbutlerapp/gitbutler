import FileListItem from '$lib/FileListItem.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Misc / FileListItem',
	component: FileListItem
} satisfies Meta<FileListItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'FileListItem'
};
