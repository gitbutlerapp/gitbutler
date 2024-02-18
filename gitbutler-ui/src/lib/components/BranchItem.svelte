<script lang="ts">
	import BranchIcon from './BranchIcon.svelte';
	// disabled until the performance issue is fixed
	// import AuthorIcons from '$lib/components/AuthorIcons.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import type { CombinedBranch } from '$lib/branches/types';
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

<a class="branch" class:selected {href}>
	{#if branch.icon}
		<BranchIcon help={branch.tooltip} name={branch.icon} color={branch.color} />
	{/if}
	<div class="branch__info flex flex-col gap-2">
		<div class="branch__details">
			<p class="text-base-13 branch__name">
				{branch.displayName}
			</p>
			<!-- <AheadBehind ahead={branch.remoteBranch?.ahead} behind={branch.remoteBranch?.behind} /> -->
		</div>
		{#if !branch.remoteBranch || branch.pr}
			<div class="branch__details">
				<span class="branch__author text-base-11 details truncate">
					<TimeAgo date={branch.modifiedAt} />
					{#if branch.author}
						by {branch.author?.name ?? 'unknown'}
					{/if}
				</span>
				<!-- <AuthorIcons authors={branch.authors} /> -->
			</div>
		{/if}
	</div>
</a>

<style lang="postcss">
	.branch {
		display: flex;
		gap: var(--space-10);
		width: 100%;

		padding: var(--space-10) var(--space-8);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);
	}

	.branch__info {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: var(--space-6);
		overflow: hidden;
	}

	.branch__details {
		display: flex;
		align-items: center;
		gap: var(--space-6);
		justify-content: space-between;
	}

	.branch__name {
		white-space: nowrap;
		overflow-x: hidden;
		text-overflow: ellipsis;
		line-height: 120%;
	}

	.branch__author {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.branch:not(.selected):hover,
	.branch:not(.selected):focus,
	.selected {
		background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
		transition: none;
	}
</style>
