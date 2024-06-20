import DemoCommitLines from './DemoCommitLines.svelte';
import type { Author } from '$lib/commitLines/types';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoCommitLines
} satisfies Meta<DemoCommitLines>;

export default meta;
type Story = StoryObj<typeof meta>;

const bill: Author = {
	email: 'bill@gitbutler.com',
	gravatarUrl: new URL('https://gravatar.com/avatar/abc123')
};

export const sameForkpointAllPopulated: Story = {
	args: {
		remoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		localCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill,
				relatedRemoteCommit: {
					id: crypto.randomUUID(),
					author: bill
				}
			}
		],
		localAndRemoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		integratedCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocals: Story = {
	args: {
		remoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		localCommits: [],
		localAndRemoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		integratedCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocalAndRemotes: Story = {
	args: {
		remoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		localCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill,
				relatedRemoteCommit: {
					id: crypto.randomUUID(),
					author: bill
				}
			}
		],
		localAndRemoteCommits: [],
		integratedCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocalAndRemotesOrIntegrateds: Story = {
	args: {
		remoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		localCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill,
				relatedRemoteCommit: {
					id: crypto.randomUUID(),
					author: bill
				}
			}
		],
		localAndRemoteCommits: [],
		integratedCommits: [],
		sameForkpoint: true
	}
};

export const sameForkpointNoRemote: Story = {
	args: {
		remoteCommits: [],
		localCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill,
				relatedRemoteCommit: {
					id: crypto.randomUUID(),
					author: bill
				}
			}
		],
		localAndRemoteCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		integratedCommits: [
			{
				id: crypto.randomUUID(),
				author: bill
			},
			{
				id: crypto.randomUUID(),
				author: bill
			}
		],
		sameForkpoint: true
	}
};
