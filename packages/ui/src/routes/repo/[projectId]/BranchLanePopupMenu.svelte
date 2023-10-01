<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { createEventDispatcher } from 'svelte';

	export let branchController: BranchController;
	export let order: number;
	export let allCollapsed: boolean | undefined;
	export let allExpanded: boolean | undefined;
	let popupMenu: PopupMenu;

	const dispatch = createEventDispatcher<{
		action: 'expand' | 'collapse' | 'generate-branch-name';
	}>();

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
	export function openByElement(elt: HTMLElement, item: any) {
		popupMenu.openByElement(elt, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item={branchId}>
	<PopupMenuItem on:click={() => branchId && branchController.unapplyBranch(branchId)}>
		Unapply
	</PopupMenuItem>

	<PopupMenuItem on:click={() => dispatch('action', 'expand')} disabled={allExpanded}
		>Expand all</PopupMenuItem
	>

	<PopupMenuItem on:click={() => dispatch('action', 'collapse')} disabled={allCollapsed}
		>Collapse all</PopupMenuItem
	>

	<PopupMenuItem on:click={() => dispatch('action', 'generate-branch-name')}
		>Generate branch name</PopupMenuItem
	>

	<div class="mx-3">
		<div class="bg-color-3 my-2 h-[0.0625rem] w-full" />
	</div>

	<PopupMenuItem on:click={() => branchController.createBranch({ order })}>
		Create branch before
	</PopupMenuItem>

	<PopupMenuItem on:click={() => branchController.createBranch({ order: order + 1 })}>
		Create branch after
	</PopupMenuItem>
</PopupMenu>
