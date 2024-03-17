<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { getContextByClass } from '$lib/utils/context';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { join } from '@tauri-apps/api/path';
	import { open } from '@tauri-apps/api/shell';
	import type { Project } from '$lib/backend/projects';
	import type { AnyFile } from '$lib/vbranches/types';

	export let project: Project | undefined;

	const branchController = getContextByClass(BranchController);

	let confirmationModal: Modal;
	let popupMenu: PopupMenu;

	function containsBinaryFiles(item: any) {
		return item.files.some((f: AnyFile) => f.binary);
	}

	function isDeleted(item: any): boolean {
		return item.files.some((f: AnyFile) => computeFileStatus(f) === 'D');
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
				{#if item.files.length === 1}
					<ContextMenuItem
						label="Copy Path"
						on:click={async () => {
							try {
								if (!project) return;
								const absPath = await join(project.path, item.files[0].path);
								navigator.clipboard.writeText(absPath);
								dismiss();
							} catch (err) {
								console.error('Failed to copy path', err);
								toasts.error('Failed to copy path');
							}
						}}
					/>
					<ContextMenuItem
						label="Copy Relative Path"
						on:click={() => {
							try {
								if (!project) return;
								navigator.clipboard.writeText(item.files[0].path);
								dismiss();
							} catch (err) {
								console.error('Failed to copy relative path', err);
								toasts.error('Failed to copy relative path');
							}
						}}
					/>
				{/if}
				<ContextMenuItem
					label="Open in VSCode"
					disabled={isDeleted(item)}
					on:click={async () => {
						try {
							if (!project) return;
							for (let file of item.files) {
								const absPath = await join(project.path, file.path);
								open(`vscode://file${absPath}`);
							}
							dismiss();
						} catch {
							console.error('Failed to open in VSCode');
							toasts.error('Failed to open in VSCode');
						}
					}}
				/>
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
		padding-left: var(--size-20);
		padding-top: var(--size-6);
	}
	.file-list li {
		padding: var(--size-2);
	}
</style>
