<script lang="ts">
	import TimeAgo from '$lib/shared/TimeAgo.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import { stringToColor } from '@gitbutler/ui/utils/stringToColor';
	import type { CombinedBranch } from '$lib/branches/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		projectId: string;
		branch: CombinedBranch;
	}

	const { projectId, branch }: Props = $props();

	let href = $derived(getBranchLink(branch));
	let selected = $state(false);

	$effect(() => {
		selected = href ? $page.url.href.endsWith(href) : false;
		// console.log(branch.authors);
	});

	function getBranchLink(b: CombinedBranch): string | undefined {
		if (b.vbranch) return `/${projectId}/board/`;
		if (b.remoteBranch) return `/${projectId}/remote/${branch?.remoteBranch?.displayName}`;
		if (b.pr) return `/${projectId}/pull/${b.pr.number}`;
	}
</script>

<button
	class="branch"
	class:selected
	onmousedown={() => {
		if (href) goto(href);
	}}
>
	<h4 class="text-base-13 text-semibold branch-name">
		{branch.displayName}
	</h4>

	<div class="row">
		<div class="branch-authors">
			{#each branch.authors as author}
				<div
					use:tooltip={author.name}
					class="author-avatar"
					style:background-color={stringToColor(author.name)}
				></div>
			{/each}
		</div>
	</div>

	{#if branch.remoteBranch || branch.pr}
		<div class="row">
			<span class="branch-author text-base-11 details truncate">
				<TimeAgo date={branch.modifiedAt} />
				{#if branch.author?.name}
					by {branch.author?.name}
				{/if}
			</span>
		</div>
	{/if}
</button>

<style lang="postcss">
	.branch {
		display: flex;
		flex-direction: column;
		padding: 10px 14px 12px 14px;
		gap: 8px;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-3);
		transition: background-color var(--transition-fast);

		&:last-child {
			border-bottom: none;
		}
	}

	.row {
		display: flex;
		align-items: center;
		gap: 6px;
		justify-content: space-between;
	}

	.branch-name {
		width: 100%;
		white-space: nowrap;
		overflow-x: hidden;
		text-overflow: ellipsis;
	}

	.branch-authors {
		display: flex;
	}

	.author-avatar {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background-color: var(--clr-scale-ntrl-50);
		margin-left: -4px;

		&:first-child {
			margin-left: 0;
		}
	}

	.branch-author {
		color: var(--clr-scale-ntrl-50);
	}

	.branch:not(.selected):hover,
	.branch:not(.selected):focus {
		background-color: var(--clr-bg-1-muted);
		transition: none;
	}

	.selected {
		background-color: var(--clr-bg-2);
	}
</style>
