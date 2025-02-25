<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	interface Props {
		projectId: string;
		commitKey: CommitKey;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		last?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		lineColor?: string;
		opacity?: number;
		borderTop?: boolean;
		onclick?: (commitId: string) => void;
	}

	const {
		commit,
		first,
		last,
		lastBranch,
		selected,
		lineColor,
		opacity,
		borderTop,
		onclick
	}: Props = $props();

	const commitTitle = $derived(commit.message.split('\n')[0]);
</script>

<button
	type="button"
	class="commit"
	class:first
	class:last
	class:selected
	style:opacity
	class:border-top={borderTop || first}
	onclick={() => onclick?.(commit.id)}
>
	<CommitLine {commit} {last} {lastBranch} {lineColor} />
	<div class="commit-content text-13 text-semibold">
		{commitTitle}
	</div>
</button>

<style>
	.commit {
		position: relative;
		display: flex;
		align-items: center;
		text-align: left;
		width: 100%;

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.border-top {
			border-top: 1px solid var(--clr-border-2);
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
		padding: 14px 14px 14px 0;
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
