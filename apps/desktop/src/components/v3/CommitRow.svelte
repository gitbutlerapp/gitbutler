<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	interface Props {
		commit: Commit | UpstreamCommit;
		first?: boolean;
		last?: boolean;
		lastBranch?: boolean;
	}

	const { first, commit, last, lastBranch }: Props = $props();
</script>

<div class="commit" class:first class:last>
	<CommitLine {commit} {last} {lastBranch} />
	<div class="commit-content text-13 text-semibold">{commit.message}</div>
</div>

<style>
	.commit {
		display: flex;
		height: 100%;

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.commit-content {
		flex: 1;
		display: flex;
		align-items: center;
	}
</style>
