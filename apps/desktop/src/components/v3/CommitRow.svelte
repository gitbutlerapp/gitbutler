<script lang="ts">
	import CommitLine from '$components/v3/CommitLine.svelte';
	import { commitPath } from '$lib/routes/routes.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	interface Props {
		projectId: string;
		commitKey: CommitKey;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		last?: boolean;
		lastBranch?: boolean;
		selected: boolean;
	}

	const { projectId, commitKey, commit, first, last, lastBranch, selected }: Props = $props();

	const commitTitle = $derived(commit.message.split('\n')[0]);
</script>

<a
	type="button"
	class="commit"
	class:first
	class:last
	class:selected
	href={commitPath(projectId, commitKey)}
>
	<CommitLine {commit} {last} {lastBranch} />
	<div class="commit-content text-13 text-semibold">
		{commitTitle}
	</div>
</a>

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
		padding: 14px 14px 14px 0;
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
