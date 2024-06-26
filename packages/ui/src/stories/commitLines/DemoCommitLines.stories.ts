import DemoCommitLines from './DemoCommitLines.svelte';
import type { Author, CommitData } from '$lib/commitLines/types';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoCommitLines
} satisfies Meta<DemoCommitLines>;

export default meta;
type Story = StoryObj<typeof meta>;

const caleb: Author = {
	email: 'hello@calebowens.com',
	gravatarUrl: new URL('https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c')
};

function author() {
	return caleb;
}

function commit(): CommitData {
	return {
		id: crypto.randomUUID(),
		title: 'This is a commit',
		author: author()
	};
}

function relatedCommit(): CommitData {
	return {
		id: crypto.randomUUID(),
		title: 'This is a commit with relations',
		author: author(),
		relatedRemoteCommit: {
			id: crypto.randomUUID(),
			title: 'This is a related commit',
			author: author()
		}
	};
}

export const sameForkpointAllPopulated: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocals: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocalAndRemotes: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), commit()],
		sameForkpoint: true
	}
};

export const sameForkpointNoLocalAndRemotesOrIntegrateds: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [],
		sameForkpoint: true
	}
};

export const sameForkpointNoRemote: Story = {
	args: {
		remoteCommits: [],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [commit()],
		integratedCommits: [commit(), commit()],
		sameForkpoint: true
	}
};

export const differentForkpointAll: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), commit()],
		sameForkpoint: false
	}
};

export const differentForkpointNoIntegrated: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [],
		sameForkpoint: false
	}
};

export const differentForkpointNoLocal: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), relatedCommit(), commit()],
		sameForkpoint: false
	}
};

export const differentForkpointNoIntegratedNoRemote: Story = {
	args: {
		remoteCommits: [],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [],
		sameForkpoint: false
	}
};

export const differentForkpointOnlyRemote: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [],
		integratedCommits: [],
		sameForkpoint: false
	}
};
