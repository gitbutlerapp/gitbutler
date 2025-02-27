<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
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
	<div class="commit-content">
		<span class="commit-title text-13 text-semibold">
			{commitTitle}
		</span>

		<div class="commit-arrow">
			<Icon name="chevron-right" />
		</div>
	</div>
</button>

<style lang="postcss">
	.commit {
		position: relative;
		display: flex;
		align-items: center;
		text-align: left;
		width: 100%;
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&::before {
			content: '';
			position: absolute;
			left: 0;
			width: 3px;
			height: 100%;
			transform: translateX(-100%);
			background-color: var(--clr-theme-pop-element);
			transition: transform var(--transition-fast);
		}

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.border-top {
			border-top: 1px solid var(--clr-border-2);
		}

		&.selected {
			background-color: var(--clr-bg-1-muted);
		}

		&.selected::before {
			transform: none;
		}

		&.selected .commit-arrow {
			margin-right: -6px;
			opacity: 0.3;
			transition:
				margin-right var(--transition-medium),
				opacity var(--transition-medium) 0.05s;
		}
	}

	.commit-content {
		display: flex;
		gap: 4px;
		align-items: center;
		width: 100%;
		padding: 14px 14px 14px 0;
		overflow: hidden;
	}

	.commit-title {
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}

	.commit-arrow {
		display: flex;
		align-items: center;
		opacity: 0;
		margin-right: -20px;
		transition:
			margin-right var(--transition-medium) 0.05s,
			opacity var(--transition-medium);
	}
</style>
