<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branchController: BranchController;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.file !== undefined}
				<ContextMenuItem label="Discard" on:click={() => branchController.unapplyFile(item.file)} />
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>
