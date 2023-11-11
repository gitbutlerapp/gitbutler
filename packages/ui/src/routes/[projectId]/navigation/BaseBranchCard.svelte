<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import IconBranch from '$lib/icons/IconBranch.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, CustomStore } from '$lib/vbranches/types';

	export let project: Project;
	export let branchController: BranchController;
	export let baseBranchStore: CustomStore<BaseBranch | undefined>;

	let baseContents: HTMLElement;
	let fetching = false;
</script>

<a
	href="/{project.id}/base"
	class="bg-color-3 flex flex-col rounded-lg p-3"
	tabindex="0"
	bind:this={baseContents}
>
	<div class="flex flex-grow items-center">
		<div class="flex flex-grow items-center gap-1">
			<span class="font-bold">{project.title}</span>
			{#if ($baseBranchStore?.behind || 0) > 0}
				<Tooltip label="Unmerged upstream commits">
					<div
						class="flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-xs font-bold text-white"
					>
						{$baseBranchStore?.behind}
					</div>
				</Tooltip>
			{/if}
		</div>
		<div class="flex">
			<Tooltip label="Fetch from upstream">
				<IconButton
					class="items-center justify-center align-top hover:bg-light-150 dark:hover:bg-dark-700"
					on:click={(e) => {
						fetching = true;
						branchController.fetchFromTarget().finally(() => (fetching = false));
						e.preventDefault();
					}}
				>
					<div class:animate-spin={fetching}>
						<IconRefresh class="h-5 w-5" />
					</div>
				</IconButton>
			</Tooltip>
		</div>
	</div>
	<div class="flex flex-grow items-center text-sm">
		<div class="flex flex-grow items-center gap-1">
			{#if $baseBranchStore?.remoteUrl.includes('github.com')}
				<IconGithub class="h-2.5 w-2.5" />
			{:else}
				<IconBranch class="h-2.5 w-2.5" />
			{/if}
			{$baseBranchStore?.branchName}
		</div>
		<div>
			<Tooltip label="Last fetch from upstream">
				{#if $baseBranchStore?.fetchedAt}
					<TimeAgo date={$baseBranchStore.fetchedAt} />
				{/if}
			</Tooltip>
		</div>
	</div>
</a>
