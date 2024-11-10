import FileIcon from '$lib/file/FileIcon.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / File Icon',
	component: FileIcon
} satisfies Meta<FileIcon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const FileIconStory: Story = {
	name: 'File Icon',
	args: {
		fileName: 'file.txt',
		size: 16
	}
};
