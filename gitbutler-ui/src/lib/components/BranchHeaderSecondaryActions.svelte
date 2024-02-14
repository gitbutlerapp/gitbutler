<script lang="ts">
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import type { Persisted } from '$lib/persisted/persisted';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch } from '$lib/vbranches/types';

	export let isLaneCollapsed: Persisted<boolean>;
	export let visible = false;

	export let isUnapplied = false;
	export let branch: Branch;
	export let branchController: BranchController;
	export let projectId: string;

	export let meatballButton: HTMLDivElement;
</script>

<div style="display: contents;">
	<Button
		icon={$isLaneCollapsed ? 'unfold-lane' : 'fold-lane'}
		kind="outlined"
		color="neutral"
		help={$isLaneCollapsed ? 'Expand lane' : 'Collapse lane'}
		on:click={() => {
			$isLaneCollapsed = !$isLaneCollapsed;
		}}
	/>
	<Button
		icon="kebab"
		kind="outlined"
		color="neutral"
		on:click={() => {
			console.log('meatballButton', meatballButton);
			visible = !visible;
		}}
	/>
	<div
		class="branch-popup-menu"
		use:clickOutside={{
			trigger: meatballButton,
			handler: () => (visible = false)
		}}
	>
		<BranchLanePopupMenu
			{branchController}
			{branch}
			{projectId}
			{isUnapplied}
			bind:visible
			on:action
		/>
	</div>
</div>

<style lang="post-css">
	.branch-popup-menu {
		position: absolute;
		top: calc(100% + var(--space-4));
		right: 0;
		z-index: 10;
	}
</style>
