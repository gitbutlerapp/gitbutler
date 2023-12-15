<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch } from '$lib/vbranches/types';
	import { fade } from 'svelte/transition';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import { clickOutside } from '$lib/clickOutside';

	export let readonly = false;
	export let branch: Branch;
	export let branchController: BranchController;
	export let projectId: string;

	let meatballButton: HTMLDivElement;
	let visible = false;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}
</script>

<div class="card__header relative" data-drag-handle>
	<div class="card__row" data-drag-handle>
		<div class="header__left">
			{#if !readonly}
				<div class="draggable" data-drag-handle>
					<Icon name="draggable" />
				</div>
			{/if}
			<BranchLabel bind:name={branch.name} on:change={handleBranchNameChange} />
		</div>
		<div class="flex items-center gap-x-1" transition:fade={{ duration: 150 }}>
			{#if !readonly}
				<div bind:this={meatballButton}>
					<IconButton icon="kebab" size="m" on:click={() => (visible = !visible)} />
				</div>
				<div
					class="branch-popup-menu"
					use:clickOutside={{ trigger: meatballButton, handler: () => (visible = false) }}
				>
					<BranchLanePopupMenu {branchController} {branch} {projectId} bind:visible on:action />
				</div>
			{/if}
		</div>
	</div>
	{#if branch.upstream}
		<div class="card__row text-base-11" data-drag-handle>
			<div class="card__remote">
				{branch.upstream.displayName}
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.card__header {
		position: relative;
		flex-direction: column;
		gap: var(--space-2);
	}
	.card__header:hover .draggable {
		color: var(--clr-theme-scale-ntrl-40);
	}
	.card__row {
		width: 100%;
		display: flex;
		justify-content: space-between;
		gap: var(--space-8);
		align-items: center;
		overflow-x: hidden;
	}
	.header__left {
		pointer-events: none;
		overflow-x: hidden;
		display: flex;
		flex-grow: 1;
		align-items: center;
		gap: var(--space-4);
	}

	.draggable {
		display: flex;
		cursor: grab;
		color: var(--clr-theme-scale-ntrl-60);
		transition: color var(--transition-medium);
	}

	.branch-popup-menu {
		position: absolute;
		top: calc(var(--space-2) + var(--space-40));
		right: var(--space-12);
		z-index: 10;
	}

	.card__remote {
		padding-left: var(--space-28);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
	}
</style>
