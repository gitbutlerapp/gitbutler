import { buildContextStore } from '$lib/utils/context';
import type { AnyCommit, DetailedCommit, RemoteCommit } from './types';

// When we can't use type for context objects we build typed getter/setter pairs
// to avoid using symbols explicitly.
export const [getLocalCommits, createLocalCommitsContextStore] =
	buildContextStore<DetailedCommit[]>('localCommits');
export const [getLocalAndRemoteCommits, createLocalAndRemoteCommitsContextStore] =
	buildContextStore<DetailedCommit[]>('remoteCommits');
export const [getIntegratedCommits, createIntegratedCommitsContextStore] =
	buildContextStore<DetailedCommit[]>('integratedCommits');
export const [getRemoteCommits, createRemoteCommitsContextStore] =
	buildContextStore<RemoteCommit[]>('remoteCommits');
export const [getCommitStore, createCommitStore] = buildContextStore<AnyCommit | undefined>(
	'commit'
);
