import { buildContextStore } from '$lib/utils/context';
import type { Commit, RemoteCommit } from './types';

// When we can't use type for context objects we build typed getter/setter pairs
// to avoid using symbols explicitly.
export const [getLocalCommits, createLocalContextStore] = buildContextStore<Commit[]>();
export const [getRemoteCommits, createRemoteContextStore] = buildContextStore<Commit[]>();
export const [getIntegratedCommits, createIntegratedContextStore] = buildContextStore<Commit[]>();
export const [getUnknownCommits, createUnknownContextStore] = buildContextStore<RemoteCommit[]>();
