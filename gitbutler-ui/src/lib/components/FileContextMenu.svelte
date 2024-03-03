<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { AnyFile } from '$lib/vbranches/types';

	export let branchController: BranchController;
	let confirmationModal: Modal;
	let popupMenu: PopupMenu;

	function containsBinaryFiles(item: any) {
		return item.files.some((f: AnyFile) => f.binary);
	}

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item let:dismiss>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.files !== undefined}
				{#if containsBinaryFiles(item)}
					<ContextMenuItem label="Discard changes (Binary files not yet supported)" disabled />
				{:else}
					<ContextMenuItem
						label="Discard changes"
						on:click={() => {
							confirmationModal.show(item);
							dismiss();
						}}
					/>
				{/if}
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>

<Modal width="small" title="Discard changes" bind:this={confirmationModal} let:item>
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
