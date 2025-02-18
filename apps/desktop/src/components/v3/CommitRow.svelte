<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	interface Props {
		commit: Commit | UpstreamCommit;
		first?: boolean;
		last?: boolean;
		lastBranch?: boolean;
		selectedCommitId?: string;
	}

	let { first, commit, last, lastBranch, selectedCommitId = $bindable() }: Props = $props();

	$inspect('commit.selectedCommitId', selectedCommitId);
</script>

<button
	type="button"
	class="commit"
	class:first
	class:last
	class:selected={selectedCommitId === commit.id}
	onclick={() => {
		if (selectedCommitId && selectedCommitId === commit.id) {
			selectedCommitId = undefined;
		} else {
			selectedCommitId = commit.id;
		}
	}}
>
	<CommitLine {commit} {last} {lastBranch} />
	<div class="commit-content text-13 text-semibold">{commit.message}</div>
</button>

<style>
	.commit {
		position: relative;
		display: flex;
		align-items: center;
		height: 100%;

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.selected::before {
			content: '';
			position: absolute;
			left: 0;
			width: 2px;
			height: 100%;
			background-color: var(--clr-theme-pop-element);
		}

		&.last.selected::before {
			border-radius: 0 0 0 var(--radius-m);
		}
	}

	.commit-content {
		flex: 1;
		display: flex;
		align-items: center;
	}
</style>
