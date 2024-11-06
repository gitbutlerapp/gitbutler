import DemoSidebarEntry from './DemoSidebarEntry.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Cards / Sidebar Entry',
	component: DemoSidebarEntry,
	argTypes: {
		selected: { control: 'boolean' },
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

const dummySeries = [
	'feature/add-user-auth',
	'bugfix/fix-login-error',
	'hotfix/update-ssl-cert',
	'feature/improve-dashboard-ui',
	'release/v1.2.0',
	'feature/refactor-api-endpoints',
	'bugfix/remove-duplicate-entries',
	'chore/update-dependencies',
	'feature/add-password-reset',
	'hotfix/correct-typo-in-readme'
];

export const SidebarEntry: Story = {
	args: {
		series: dummySeries,
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
		series: dummySeries,
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
			title: 'bestest pr',
			draft: false
		}
	}
};

export const SidebarEntryInWorkspace: Story = {
	args: {
		series: dummySeries,
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
			title: 'bestest pr',
			draft: true
		}
	}
};
