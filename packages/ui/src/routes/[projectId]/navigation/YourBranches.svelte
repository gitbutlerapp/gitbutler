<script lang="ts">
	import { IconTriangleDown } from '$lib/icons';
	import { accordion } from './accordion';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import IconChevronRightSmall from '$lib/icons/IconChevronRightSmall.svelte';
	import { page } from '$app/stores';
	import { slide } from 'svelte/transition';
	import { computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import type { Branch } from '$lib/vbranches/types';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Project } from '$lib/backend/projects';
	import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	export let vbranchService: VirtualBranchService;
	export let branchController: BranchController;
	export let project: Project;

	$: branches$ = vbranchService.branches$;
	$: branchesError$ = vbranchService.branchesError$;

	let yourBranchesOpen = true;
	let vbViewport: HTMLElement;
	let vbContents: HTMLElement;

	let applyConflictedModal: Modal;

	function sumBranchLinesAddedRemoved(branch: Branch) {
		const comitted = computedAddedRemoved(...branch.commits.flatMap((c) => c.files));
		const uncomitted = computedAddedRemoved(...branch.files);

		return {
			added: comitted.added + uncomitted.added,
			removed: comitted.removed + uncomitted.removed
		};
	}

	function toggleBranch(branch: Branch) {
		if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
	}
</script>

<div
	class="flex items-center justify-between border-b border-t px-2 py-1 pr-1"
	style:background-color="var(--bg-surface-highlight)"
	style:border-color="var(--border-surface)"
>
	<div class="flex flex-row place-items-center space-x-2">
		<button class="h-full w-full" on:click={() => (yourBranchesOpen = !yourBranchesOpen)}>
			<IconTriangleDown class={!yourBranchesOpen ? '-rotate-90' : ''} />
		</button>
		<div class="whitespace-nowrap font-bold">Stashed branches</div>
	</div>
	<div class="flex h-4 w-4 justify-around"></div>
</div>
<div
	use:accordion={yourBranchesOpen}
	style:height={`${$userSettings.vbranchExpandableHeight}px`}
	class="relative shrink-0"
>
	<div
		bind:this={vbViewport}
		class="hide-native-scrollbar flex h-full max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={vbContents}>
			{#if $branchesError$}
				<div class="px-2 py-1">Something went wrong!</div>
			{:else if !$branches$}
				<div class="px-2 py-1">Loading...</div>
			{:else if !$branches$ || $branches$.length == 0}
				<div class="text-color-2 p-2">You currently have no virtual branches</div>
			{:else if $branches$.filter((b) => !b.active).length == 0}
				<div class="text-color-2 p-2">You have no stashed branches</div>
			{:else}
				{#each $branches$.filter((b) => !b.active) as branch (branch.id)}
					{@const { added, removed } = sumBranchLinesAddedRemoved(branch)}
					{@const latestModifiedAt = branch.files.at(0)?.hunks.at(0)?.modifiedAt}
					<a
						href={`/${project.id}/stashed/${branch.id}`}
						transition:slide={{ duration: 250 }}
						class="group block border-b p-2 pr-0 -outline-offset-2 outline-blue-200 last:border-b focus-within:outline-2"
						class:bg-light-50={$page.url.pathname.includes(branch.id)}
						class:dark:bg-zinc-700={$page.url.pathname.includes(branch.id)}
						style:border-color="var(--border-surface)"
					>
						<div class="relative flex max-w-full flex-row">
							<div class="flex flex-shrink flex-grow flex-col gap-y-2 overflow-hidden">
								<div class="text-color-2 flex-grow truncate">
									{branch.name}
								</div>
								<div class="flex shrink-0 items-baseline gap-x-2 text-sm">
									{#if latestModifiedAt}
										<span class="text-color-4"><TimeAgo date={latestModifiedAt} /></span>
									{/if}
									<div class="flex gap-1 font-mono text-sm font-bold">
										<span class="font-mono text-green-500">
											+{added}
										</span>
										<span class="font-mono text-red-500">
											-{removed}
										</span>
									</div>
									{#await branch.isMergeable then isMergeable}
										{#if !branch.active}
											{#if !branch.baseCurrent}
												<!-- branch will cause merge conflicts if applied -->
												<Tooltip label="Will introduce merge conflicts if applied">
													<span class="text-yellow-500">&#9679;</span>
												</Tooltip>
											{:else if !isMergeable}
												<Tooltip
													label="Canflicts with changes in your working directory, cannot be applied"
												>
													<span class="text-red-500">&#9679;</span>
												</Tooltip>
											{:else if isMergeable && (added > 0 || removed > 0)}
												<Tooltip label="Can be applied cleanly">
													<span class="text-green-500">&#9679;</span>
												</Tooltip>
											{/if}
										{/if}
									{/await}
								</div>
							</div>
							<div
								class="shrink-0 self-center overflow-hidden whitespace-nowrap px-2 opacity-0 transition-opacity group-hover:opacity-100 group-focus:opacity-100"
							>
								<IconButton
									icon={IconChevronRightSmall}
									class="text-color-4 hover:text-color-3 flex items-center gap-x-2 p-0 text-sm font-semibold"
									title="apply branch"
									on:click={() => {
										toggleBranch(branch);
									}}
								>
									Apply
								</IconButton>
							</div>
						</div>
					</a>
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar viewport={vbViewport} contents={vbContents} width="0.5rem" />
</div>

<Resizer
	minHeight={200}
	viewport={vbViewport}
	direction="vertical"
	class="z-30"
	on:height={(e) => {
		userSettings.update((s) => ({
			...s,
			vbranchExpandableHeight: e.detail
		}));
	}}
/>

<Modal width="small" bind:this={applyConflictedModal}>
	<svelte:fragment slot="title">Merge conflicts</svelte:fragment>
	<p>Applying this branch will introduce merge conflicts.</p>
	<svelte:fragment slot="controls" let:item let:close>
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="purple"
			on:click={() => {
				branchController.applyBranch(item.id);
				close();
			}}
		>
			Update
		</Button>
	</svelte:fragment>
</Modal>
