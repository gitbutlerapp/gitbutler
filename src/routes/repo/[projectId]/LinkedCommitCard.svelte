<script lang="ts">
	import type { Commit } from '$lib/vbranches';
	import CommitCard from './CommitCard.svelte';

	export let commit: Commit;
	export let url: string | undefined;
</script>

<a href={url} target="_blank" title="Open in browser" class="flex-grow overflow-x-hidden">
	<CommitCard {commit} />
</a>
