import { buildContextStore } from '$lib/utils/context';
import type { AnyCommit, Commit, RemoteCommit } from './types';

// When we can't use type for context objects we build typed getter/setter pairs
// to avoid using symbols explicitly.
export const [getLocalCommits, createLocalCommitsContextStore] =
	buildContextStore<Commit[]>('localCommits');
export const [getLocalAndRemoteCommits, createLocalAndRemoteCommitsContextStore] =
	buildContextStore<Commit[]>('remoteCommits');
export const [getIntegratedCommits, createIntegratedCommitsContextStore] =
	buildContextStore<Commit[]>('integratedCommits');
export const [getRemoteCommits, createRemoteCommitsContextStore] =
	buildContextStore<RemoteCommit[]>('remoteCommits');
export const [getCommitStore, createCommitStore] = buildContextStore<AnyCommit | undefined>(
	'commit'
);
