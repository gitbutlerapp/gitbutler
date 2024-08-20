import DemoSidebarEntry from './DemoSidebarEntry.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Cards / Sidebar Entry',
	component: DemoSidebarEntry,
	argTypes: {
		selected: { control: 'boolean' },
		title: { control: 'text' },
		applied: { control: 'boolean' },
		pullRequestDetails: { control: 'object' },
		lastCommitDetails: { control: 'object' },
		branchDetails: { control: 'object' },
		remotes: { control: 'object' },
		local: { control: 'boolean' }
	}
} satisfies Meta<DemoSidebarEntry>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SidebarEntry: Story = {
	args: {
		title: 'best branch ever',
		selected: false,
		applied: false,
		pullRequestDetails: undefined,
		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: '2024-07-31T15:39:14.540Z'
		},
		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},
		remotes: [],
		local: true
	}
};

export const SidebarEntryPr: Story = {
	args: {
		title: 'best branch ever',
		selected: false,
		applied: false,

		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: '2024-07-31T15:39:14.540Z'
		},

		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},

		remotes: ['origin'],
		local: true,

		pullRequestDetails: {
			title: 'bestest pr'
		}
	}
};

export const SidebarEntryInWorkspace: Story = {
	args: {
		title: 'best branch ever',
		selected: false,
		applied: true,

		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: '2024-07-31T15:39:14.540Z'
		},

		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},

		remotes: ['origin'],
		local: true,

		pullRequestDetails: {
			title: 'bestest pr'
		}
	}
};
