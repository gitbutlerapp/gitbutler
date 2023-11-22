<script lang="ts">
	import { accordion } from './accordion';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { slide } from 'svelte/transition';
	import Modal from '$lib/components/Modal.svelte';
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Project } from '$lib/backend/projects';
	import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';
	import type { UIEventHandler } from 'svelte/elements';
	import SectionHeader from './SectionHeader.svelte';
	import Icon from '$lib/icons/Icon.svelte';

	export let vbranchService: VirtualBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let expanded = true;

	$: branches$ = vbranchService.branches$;
	$: branchesError$ = vbranchService.branchesError$;

	let viewport: HTMLElement;
	let contents: HTMLElement;

	let applyConflictedModal: Modal;

	let scrolled: boolean;
	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};
</script>

<SectionHeader {scrolled} count={$branches$?.length ?? 0} expandable={true} bind:expanded>
	Stashed branches
</SectionHeader>
<div use:accordion={expanded} class="container relative flex-grow">
	<div
		bind:this={viewport}
		on:scroll={onScroll}
		class="viewport hide-native-scrollbar flex h-full max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={contents} class="contents">
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
					<a
						class="item"
						href={`/${project.id}/stashed/${branch.id}`}
						transition:slide={{ duration: 250 }}
					>
						<Icon name="branch" />
						<div class="text-color-2 flex-grow truncate">
							{branch.name}
						</div>
					</a>
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar {viewport} {contents} width="0.5rem" />
</div>

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

<style>
	.viewport {
		padding-top: var(--space-4);
		padding-bottom: var(--space-4);
		padding-left: var(--space-16);
		padding-right: var(--space-16);
	}
	.container {
		min-height: 10rem;
	}
	.item {
		display: flex;
		gap: var(--space-10);
		padding-top: var(--space-10);
		padding-bottom: var(--space-10);
		padding-left: var(--space-8);
		padding-right: var(--space-8);
		border-radius: var(--radius-m);
	}
	.item:hover,
	.item:focus {
		background-color: var(--clr-theme-container-pale);
	}
</style>
