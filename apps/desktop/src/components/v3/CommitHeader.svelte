<script lang="ts">
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	type Props = {
		row?: boolean;
		commit: UpstreamCommit | Commit;
		class?: string;
	};

	const { commit, row, class: className }: Props = $props();

	const message = $derived(commit.message);
	const indexOfNewLine = $derived(message.indexOf('\n'));
	const endIndex = $derived(indexOfNewLine === -1 ? message.length : indexOfNewLine + 1);
	const title = $derived(message.slice(0, endIndex).trim());
</script>

<h3 class="{className} commit-title" class:row>
	{title}
</h3>

<style>
	.commit-title {
		flex-grow: 1;
	}

	.row {
		text-align: left;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
