<script lang="ts">
	import { splitMessage } from '$lib/utils/commitMessage';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	type Props = {
		row?: boolean;
		commit: UpstreamCommit | Commit;
		class?: string;
	};

	const { commit, row, class: className }: Props = $props();

	const title = $derived(splitMessage(commit.message).title);
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
