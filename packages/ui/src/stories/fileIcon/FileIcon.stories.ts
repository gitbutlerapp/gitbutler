import FileIcon from '$lib/FileIcon.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Files / File Icon',
	component: FileIcon
} satisfies Meta<FileIcon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const IconStory: Story = {
	name: 'File Icon',
	args: {
		fileName: 'file.txt'
	}
};
