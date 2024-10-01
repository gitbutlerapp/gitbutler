import DemoFileListItem from './DemoFileListItem.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Files / FileListItem',
	component: DemoFileListItem
} satisfies Meta<DemoFileListItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const FileListItemStory: Story = {
	name: 'FileListItem',
	args: {
		filePath: '/path/to/file.svelte',
		fileStatus: 'A',
		fileStatusStyle: 'dot',
		clickable: true,
		selected: false,
		conflicted: true,
		draggable: true,
		showCheckbox: true,
		checked: true,
		lockText: 'Locked by someone',
		onclick: () => {
			console.log('clicked');
		},
		oncheck: (e: Event) => {
			console.log('checked', e);
		}
	}
};
