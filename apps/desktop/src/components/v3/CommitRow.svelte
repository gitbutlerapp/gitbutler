<script lang="ts">
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	interface Props {
		commit: Commit | UpstreamCommit;
		first?: boolean;
		last?: boolean;
	}

	const { first, last, commit }: Props = $props();
</script>

<div class="commit" class:first class:last>
	<span class="text-13 text-semibold">{commit.message}</span>
</div>

<style>
	.commit {
		padding: 8px;
	}
</style>
