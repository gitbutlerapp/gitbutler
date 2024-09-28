import DemoCommitLines from './DemoCommitLines.svelte';
import type { Author, CommitData } from '$lib/commitLinesStacking/types';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Commit Lines Stacking/ Variants',
	component: DemoCommitLines
} satisfies Meta<DemoCommitLines>;

export default meta;
type Story = StoryObj<typeof meta>;

const caleb: Author = {
	email: 'hello@calebowens.com',
	gravatarUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c'
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

export const allPopulated: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), commit(), relatedCommit(), relatedCommit()],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()]
	}
};

export const noLocals: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()]
	}
};

export const noLocalAndRemotes: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), commit()]
	}
};

export const noLocalAndRemotesOrIntegrateds: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}
};

export const noRemote: Story = {
	args: {
		remoteCommits: [],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [commit()],
		integratedCommits: [commit(), commit()]
	}
};

export const noIntegrated: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}
};

export const localAndShadowOnly: Story = {
	args: {
		remoteCommits: [],
		localCommits: [relatedCommit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}
};

export const onlyRemote: Story = {
	args: {
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [],
		integratedCommits: []
	}
};
