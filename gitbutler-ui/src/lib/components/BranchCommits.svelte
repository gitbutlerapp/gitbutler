<script lang="ts">
	import CommitList from './CommitList.svelte';
	import {
		getIntegratedCommits,
		getLocalCommits,
		getRemoteCommits,
		getUnknownCommits
	} from '$lib/vbranches/contexts';

	export let isUnapplied: boolean;

	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const integratedCommits = getIntegratedCommits();
	const unknownCommits = getUnknownCommits();
</script>

{#if $unknownCommits && $unknownCommits.length > 0}
	<CommitList {isUnapplied} commits={$unknownCommits} type="upstream" />
{/if}
<CommitList {isUnapplied} type="local" commits={$localCommits} />
<CommitList {isUnapplied} type="remote" commits={$remoteCommits} />
<CommitList {isUnapplied} type="integrated" commits={$integratedCommits} />
