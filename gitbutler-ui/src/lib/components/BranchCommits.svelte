<script lang="ts">
	import CommitList from './CommitList.svelte';
	import { getContextStoreBySymbol } from '$lib/utils/context';
	import {
		type AnyFile,
		Commit,
		LOCAL_COMMITS,
		REMOTE_COMMITS,
		INTEGRATED_COMMITS,
		UNKNOWN_COMMITS
	} from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;

	const localCommits = getContextStoreBySymbol<Commit[]>(LOCAL_COMMITS);
	const remoteCommits = getContextStoreBySymbol<Commit[]>(REMOTE_COMMITS);
	const integratedCommits = getContextStoreBySymbol<Commit[]>(INTEGRATED_COMMITS);
	const unknownCommits = getContextStoreBySymbol<Commit[]>(UNKNOWN_COMMITS);
</script>

{#if $unknownCommits && $unknownCommits.length > 0}
	<CommitList {isUnapplied} {selectedFiles} commits={$unknownCommits} type="upstream" />
{/if}
<CommitList {isUnapplied} {selectedFiles} type="local" commits={$localCommits} />
<CommitList {isUnapplied} {selectedFiles} type="remote" commits={$remoteCommits} />
<CommitList {isUnapplied} {selectedFiles} type="integrated" commits={$integratedCommits} />
