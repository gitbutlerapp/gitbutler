<script lang="ts">
	import CommitList from './CommitList.svelte';
	import {
		getIntegratedCommits,
		getLocalCommits,
		getRemoteCommits,
		getUnknownCommits
	} from '$lib/vbranches/contexts';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;

	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const integratedCommits = getIntegratedCommits();
	const unknownCommits = getUnknownCommits();
</script>

{#if $unknownCommits && $unknownCommits.length > 0}
	<CommitList {isUnapplied} {selectedFiles} commits={$unknownCommits} type="upstream" />
{/if}
<CommitList {isUnapplied} {selectedFiles} type="local" commits={$localCommits} />
<CommitList {isUnapplied} {selectedFiles} type="remote" commits={$remoteCommits} />
<CommitList {isUnapplied} {selectedFiles} type="integrated" commits={$integratedCommits} />
