<script lang="ts">
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	type Props = {
		row?: boolean;
		commit: UpstreamCommit | Commit;
	};

	const { commit, row }: Props = $props();

	const message = $derived(commit.message);
	const indexOfNewLine = $derived(message.indexOf('\n'));
	const endIndex = $derived(indexOfNewLine === -1 ? message.length : indexOfNewLine + 1);
	const title = $derived(message.slice(0, endIndex).trim());
</script>

<div class="commit-title text-14 text-semibold" class:row>
	{title}
</div>

<style>
	.commit-title {
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
