<script lang="ts">
	import { page } from '$app/stores';
	import type { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import Icon from '$lib/icons/Icon.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	export let project: Project;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;
	export let prService: PrService;

	$: base$ = baseBranchService.base$;
	$: selected = $page.url.href.endsWith('/base');

	let baseContents: HTMLElement;
	let fetching = false;
	let loading = false;
</script>

<a
	href="/{project.id}/base"
	class="relative flex flex-grow items-center gap-x-2 rounded-md px-3 py-1 text-lg"
	style:background-color={selected ? 'var(--bg-surface-highlight)' : undefined}
	bind:this={baseContents}
>
	<div class="flex items-center gap-1">
		{#if $base$?.remoteUrl.includes('github.com')}
			<IconGithub class="h-4 w-4" />
		{:else}
			<Icon name="branch" />
		{/if}
	</div>
	<div class="font-semibold">
		{$base$?.branchName}
	</div>

	{#if ($base$?.behind || 0) > 0}
		<Tooltip label="Unmerged upstream commits">
			<div
				class="flex h-4 w-4 items-center justify-center rounded-full text-base font-bold"
				style:background-color="var(--bg-surface-highlight)"
			>
				{$base$?.behind}
			</div>
		</Tooltip>
		<Button
			height="small"
			color="purple"
			{loading}
			on:click={async (e) => {
				e.preventDefault();
				e.stopPropagation();
				loading = true;
				try {
					await branchController.updateBaseBranch();
				} finally {
					loading = false;
				}
			}}
		>
			update
		</Button>
	{/if}
	<IconButton
		class="items-center justify-center align-top "
		on:click={async (e) => {
			e.preventDefault();
			e.stopPropagation();
			fetching = true;
			await branchController.fetchFromTarget().finally(() => {
				fetching = false;
				prService.reload();
			});
		}}
	>
		<div class:animate-spin={fetching}>
			<IconRefresh class="h-4 w-4" />
		</div>
	</IconButton>
</a>
<div class="text-color-3 py-0.5 pl-9 text-sm">
	<Tooltip label="Last fetch from upstream">
		{#if $base$?.fetchedAt}
			<TimeAgo date={$base$.fetchedAt} />
		{/if}
	</Tooltip>
</div>
