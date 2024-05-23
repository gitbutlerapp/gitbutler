import { buildContextStore } from '$lib/utils/context';
import type { AnyCommit, AnyFile, Commit, RemoteCommit } from './types';
import type { Writable } from 'svelte/store';

// When we can't use type for context objects we build typed getter/setter pairs
// to avoid using symbols explicitly.
export const [getLocalCommits, createLocalContextStore] =
	buildContextStore<Commit[]>('localCommits');
export const [getRemoteCommits, createRemoteContextStore] =
	buildContextStore<Commit[]>('remoteCommits');
export const [getUpstreamCommits, createUpstreamContextStore] =
	buildContextStore<RemoteCommit[]>('upstreamCommits');
export const [getUnknownCommits, createUnknownCommitsStore] =
	buildContextStore<RemoteCommit[]>('unknownCommits');
export const [getSelectedFiles, createSelectedFiles] = buildContextStore<
	AnyFile[],
	Writable<AnyFile[]>
>('selectedFiles');
export const [getCommitStore, createCommitStore] = buildContextStore<AnyCommit | undefined>(
	'commit'
);
