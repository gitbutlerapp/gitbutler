<script lang="ts">
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { getNameNormalizationServiceContext } from '$lib/branches/nameNormalizationService';
	import Button from '$lib/shared/Button.svelte';
	import { getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranch } from '$lib/vbranches/types';

	export let isUnapplied = false;
	export let hasIntegratedCommits = false;
	export let isLaneCollapsed: boolean;
	export let remoteExists: boolean;

	const baseBranch = getContextStore(BaseBranch);
	const branch = getContextStore(VirtualBranch);

	const nameNormalizationService = getNameNormalizationServiceContext();

	let normalizedBranchName: string;

	$: if ($branch.displayName) {
		nameNormalizationService
			.normalize($branch.displayName)
			.then((name) => {
				normalizedBranchName = name;
			})
			.catch((e) => {
				console.error('Failed to normalize branch name', e);
			});
	}
</script>

{#if !remoteExists}
	{#if hasIntegratedCommits}
		<Button
			clickable={false}
			size="tag"
			icon="pr-small"
			style="success"
			kind="solid"
			help="These changes have been integrated upstream, update your workspace to make this lane disappear."
			reversedDirection>Integrated</Button
		>
	{:else}
		<Button
			clickable={false}
			size="tag"
			icon="virtual-branch-small"
			style="neutral"
			help="These changes are in your working directory."
			reversedDirection>Virtual</Button
		>
	{/if}
	{#if !isUnapplied && !isLaneCollapsed}
		{#await normalizedBranchName then name}
			<Button
				clickable={false}
				size="tag"
				style="neutral"
				shrinkable
				disabled
				help="Branch name that will be used when pushing. You can change it from the lane menu."
			>
				{name}
			</Button>
		{/await}
	{/if}
{:else}
	<Button
		clickable={false}
		size="tag"
		style="neutral"
		kind="solid"
		icon="remote-branch-small"
		help="At least some of your changes have been pushed"
		reversedDirection>Remote</Button
	>
	<Button
		size="tag"
		icon="open-link"
		style="ghost"
		outline
		shrinkable
		on:click={(e) => {
			const url = $baseBranch?.branchUrl($branch.upstream?.name);
			if (url) openExternalUrl(url);
			e.preventDefault();
			e.stopPropagation();
		}}
	>
		{isLaneCollapsed ? 'View branch' : $branch.displayName}
	</Button>
{/if}
