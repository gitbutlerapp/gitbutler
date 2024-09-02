<script lang="ts">
	import { getNameNormalizationServiceContext } from '$lib/branches/nameNormalizationService';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';

	const {
		isUnapplied = false,
		hasIntegratedCommits = false,
		isLaneCollapsed,
		remoteExists
	}: {
		isUnapplied?: boolean;
		hasIntegratedCommits?: boolean;
		isLaneCollapsed: boolean;
		remoteExists: boolean;
	} = $props();

	const branch = getContextStore(VirtualBranch);
	const upstreamName = $derived($branch.upstreamName);
	const gitHost = getGitHost();
	const gitHostBranch = $derived(upstreamName ? $gitHost?.branch(upstreamName) : undefined);

	const nameNormalizationService = getNameNormalizationServiceContext();

	let normalizedBranchName: string | undefined = $state();

	$effect(() => {
		nameNormalizationService
			.normalize($branch.displayName)
			.then((name) => {
				normalizedBranchName = name;
			})
			.catch((e) => {
				console.error('Failed to normalize branch name', e);
			});
	});
</script>

{#if !remoteExists}
	{#if hasIntegratedCommits}
		<Button
			clickable={false}
			size="tag"
			icon="pr-small"
			style="success"
			kind="solid"
			tooltip="Changes have been integrated upstream, update your workspace to make this lane disappear."
			reversedDirection>Integrated</Button
		>
	{:else}
		<Button
			clickable={false}
			size="tag"
			icon="virtual-branch-small"
			style="neutral"
			tooltip="Changes are in your working directory"
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
				tooltip={'Branch name that will be used when pushing.\nChange it from the lane menu'}
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
		tooltip="Some changes have been pushed"
		reversedDirection>Remote</Button
	>
	<Button
		size="tag"
		icon="open-link"
		style="ghost"
		outline
		shrinkable
		onclick={(e) => {
			const url = gitHostBranch?.url;
			if (url) openExternalUrl(url);
			e.preventDefault();
			e.stopPropagation();
		}}
	>
		{isLaneCollapsed ? 'View branch' : $branch.displayName}
	</Button>
{/if}
