<script lang="ts">
	import BranchIcon from '../branch/BranchIcon.svelte';
	import TimeAgo from '$lib/shared/TimeAgo.svelte';
	import type { CombinedBranch } from '$lib/branches/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let projectId: string;
	export let branch: CombinedBranch;

	function getBranchLink(b: CombinedBranch): string | undefined {
		if (b.vbranch?.active) return `/${projectId}/board/`;
		if (b.vbranch) return `/${projectId}/stashed/${b.vbranch.id}`;
		if (b.remoteBranch) return `/${projectId}/remote/${branch?.remoteBranch?.sha}`;
		if (b.pr) return `/${projectId}/pull/${b.pr.number}`;
	}

	$: href = getBranchLink(branch);
	$: selected = href ? $page.url.href.endsWith(href) : false;
</script>

<button
	class="branch"
	class:selected
	on:mousedown={() => {
		if (href) goto(href);
	}}
>
	{#if branch.icon}
		<BranchIcon help={branch.tooltip} name={branch.icon} />
	{/if}
	<div class="branch__info">
		<div class="branch__details">
			<p class="text-base-13 branch__name">
				{branch.displayName}
			</p>
		</div>
		{#if branch.remoteBranch || branch.pr}
			<div class="branch__details">
				<span class="branch__author text-base-11 details truncate">
					<TimeAgo date={branch.modifiedAt} />
					{#if branch.author}
						by {branch.author?.name ?? 'unknown'}
					{/if}
				</span>
			</div>
		{/if}
	</div>
</button>

<style lang="postcss">
	.branch {
		display: flex;
		gap: 10px;
		width: 100%;

		padding: 10px 8px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);
	}

	.branch__info {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: 6px;
		overflow: hidden;
	}

	.branch__details {
		display: flex;
		align-items: center;
		gap: 6px;
		justify-content: space-between;
	}

	.branch__name {
		white-space: nowrap;
		overflow-x: hidden;
		text-overflow: ellipsis;
		line-height: 120%;
	}

	.branch__author {
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
