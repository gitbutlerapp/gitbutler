<script lang="ts">
	import Tag from '$lib/components/Tag.svelte';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { openExternalUrl } from '$lib/utils/url';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';

	export let base: BaseBranch | undefined | null;
	export let branch: Branch;
	export let isUnapplied = false;
	export let hasIntegratedCommits = false;
	export let isLaneCollapsed: boolean;
</script>

{#if !branch.upstream}
	{#if !branch.active}
		<Tag
			icon="virtual-branch-small"
			color="light"
			help="These changes are stashed away from your working directory."
			reversedDirection
			verticalOrientation={isLaneCollapsed}>unapplied</Tag
		>
	{:else if hasIntegratedCommits}
		<Tag
			icon="pr-small"
			color="success"
			help="These changes have been integrated upstream, update your workspace to make this lane disappear."
			reversedDirection
			verticalOrientation={isLaneCollapsed}>Integrated</Tag
		>
	{:else}
		<Tag
			icon="virtual-branch-small"
			color="light"
			help="These changes are in your working directory."
			reversedDirection
			verticalOrientation={isLaneCollapsed}>Virtual</Tag
		>
	{/if}
	{#if !isUnapplied && !isLaneCollapsed}
		<Tag
			shrinkable
			disabled
			help="Branch name that will be used when pushing. You can change it from the lane menu."
			verticalOrientation={isLaneCollapsed}
		>
			origin/{branch.upstreamName ? branch.upstreamName : normalizeBranchName(branch.name)}</Tag
		>
	{/if}
{:else}
	<Tag
		color="dark"
		icon="remote-branch-small"
		help="At least some of your changes have been pushed"
		verticalOrientation={isLaneCollapsed}
		reversedDirection>Remote</Tag
	>
	<Tag
		icon="open-link"
		color="ghost"
		border
		clickable
		shrinkable
		verticalOrientation={isLaneCollapsed}
		on:click={(e) => {
			const url = base?.branchUrl(branch.upstream?.name);
			if (url) openExternalUrl(url);
			e.preventDefault();
			e.stopPropagation();
		}}
	>
		{isLaneCollapsed ? 'View branch' : `origin/${branch.upstream?.name}`}
	</Tag>
{/if}
