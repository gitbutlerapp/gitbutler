<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	interface Props {
		projectId: string;
		commitKey: CommitKey;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		lineColor?: string;
		opacity?: number;
		borderTop?: boolean;
	}

	const {
		commit,
		first,
		lastCommit,
		lastBranch,
		selected,
		lineColor,
		opacity,
		borderTop,
		href
	}: Props = $props();

</script>

<div
	role="listitem"
	class="commit"
	class:first
	class:lastCommit
	class:selected
	style:opacity
	class:border-top={borderTop || first}
	{href}
>
	<CommitLine {commit} {lastCommit} {lastBranch} {lineColor} />
	<div class="commit-content">
		<span class="commit-title text-13 text-semibold">
			{commitTitle}
		</span>

		<div class="commit-arrow">
			<Icon name="chevron-right" />
		</div>
	</div>
</div>

<style lang="postcss">
	.commit {
		position: relative;
		display: flex;
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

		&.border-top {
			/* border-top: 1px solid var(--clr-border-2); */
		}

		&.selected {
			/* background-color: var(--clr-bg-1-muted); */
		}

		&.selected::before {
			transform: none;
		}
	}

	.commit-content {
		display: flex;
		flex-direction: column;
		position: relative;
		gap: 6px;
		width: 100%;
		padding: 14px 14px 14px 0;
		overflow: hidden;
	}
</style>
