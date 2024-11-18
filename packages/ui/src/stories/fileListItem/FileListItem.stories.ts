import DemoFileListItem from './DemoFileListItem.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'List items / FileListItem',
	component: DemoFileListItem as any
} satisfies Meta<typeof DemoFileListItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const FileListItemStory: Story = {
	name: 'Default',
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

export const OnResolveStory: Story = {
	name: 'Resolve button',
	args: {
		filePath: '/path/to/file.svelte',
		fileStatus: 'A',
		fileStatusStyle: 'dot',
		clickable: false,
		selected: false,
		conflicted: true,
		checked: true,
		onclick: () => {
			console.log('clicked');
		},
		onresolveclick: () => {
			console.log('resolve clicked');
		}
	}
};
