<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch } from '$lib/vbranches/types';
	import { fade } from 'svelte/transition';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import type { Writable } from 'svelte/store';
	import { createEventDispatcher, onDestroy, onMount } from 'svelte';

	export let readonly = false;
	export let branch: Branch;
	export let allExpanded: Writable<boolean>;
	export let allCollapsed: Writable<boolean>;
	export let branchController: BranchController;
	export let projectId: string;

	const dispatch = createEventDispatcher<{ action: string }>();
	let meatballButton: HTMLDivElement;

	// We have to create this manually for now.
	// TODO: Use document.body.addEventListener to avoid having to use backdrop
	let popupMenu = new BranchLanePopupMenu({
		target: document.body,
		props: { allExpanded, allCollapsed, order: branch?.order, branchController, projectId }
	});

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	onMount(() => {
		return popupMenu.$on('action', (e) => {
			dispatch('action', e.detail);
		});
	});

	onDestroy(() => {
		popupMenu.$destroy();
	});
</script>

<div class="header">
	<div class="header__left flex-grow">
		{#if !readonly}
			<div class="draggable" id="drag-handle">
				<Icon name="draggable" />
			</div>
		{/if}
		<BranchLabel bind:name={branch.name} on:change={handleBranchNameChange} />
	</div>
	<div class="flex items-center gap-x-1" transition:fade={{ duration: 150 }}>
		{#if !readonly}
			<div bind:this={meatballButton}>
				<IconButton
					icon="kebab"
					size="m"
					on:click={() => popupMenu.openByElement(meatballButton, branch.id)}
				/>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.header {
		display: flex;
		width: 100%;
		align-items: center;
		padding: var(--space-12);
		gap: var(--space-8);
		&:hover .draggable {
			color: var(--clr-theme-scale-ntrl-40);
		}
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
	.header__left {
		display: flex;
		gap: var(--space-4);
		align-items: center;
	}
	.draggable {
		cursor: grab;
		color: var(--clr-theme-scale-ntrl-60);
	}
</style>
