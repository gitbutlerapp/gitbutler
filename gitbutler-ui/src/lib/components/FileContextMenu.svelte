<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branchController: BranchController;
	let confirmationModal: Modal;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.files !== undefined}
				<ContextMenuItem label="Discard" on:click={() => confirmationModal.show(item)} />
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>

<Modal width="small" title="Discard file" bind:this={confirmationModal} let:item>
	<div>
		Discarding changes to the following files:
		<ul class="file-list">
			{#each item.files as file}
				<li><code>{file.path}</code></li>
			{/each}
		</ul>
	</div>
	<svelte:fragment slot="controls" let:close let:item>
		<Button kind="outlined" color="neutral" on:click={close}>Cancel</Button>
		<Button
			color="error"
			on:click={() => {
				branchController.unapplyFiles(item.files);
				confirmationModal.close();
			}}
		>
			Confirm
		</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.file-list {
		list-style: disc;
		padding-left: var(--space-20);
		padding-top: var(--space-6);
	}
	.file-list li {
		padding: var(--space-2);
	}
</style>
