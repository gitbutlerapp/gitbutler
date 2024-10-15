<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { editor } from '$lib/editorLink/editorLink';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { isAnyFile, LocalFile } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { join } from '@tauri-apps/api/path';

	export let branchId: string | undefined;
	export let target: HTMLElement | undefined;
	export let isUnapplied;

	const branchController = getContext(BranchController);
	const project = getContext(Project);

	let confirmationModal: Modal;
	let contextMenu: ReturnType<typeof ContextMenu>;

	function isDeleted(item: any): boolean {
		if (!item.files || !Array.isArray(item.files)) return false;

		return item.files.some((f: unknown) => {
			if (!isAnyFile(f)) return false;
			computeFileStatus(f) === 'D';
		});
	}

	function confirmDiscard(item: any) {
		if (!branchId) {
			console.error('Branch ID is not set');
			toasts.error('Failed to discard changes');
			return;
		}
		branchController.unapplyFiles(branchId, item.files);
		close();
	}

	export function open(e: MouseEvent, item: any) {
		contextMenu.open(e, item);
	}
</script>

<ContextMenu bind:this={contextMenu} {target} openByMouse>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.files && item.files.length > 0}
				{@const files = item.files}
				{#if files[0] instanceof LocalFile && !isUnapplied}
					<ContextMenuItem
						label="Discard changes"
						on:click={() => {
							confirmationModal.show(item);
							contextMenu.close();
						}}
					/>
				{/if}
				{#if files.length === 1}
					<ContextMenuItem
						label="Copy Path"
						on:click={async () => {
							try {
								if (!project) return;
								const absPath = await join(project.path, item.files[0].path);
								navigator.clipboard.writeText(absPath);
								contextMenu.close();
								// dismiss();
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
								contextMenu.close();
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
								const absPath = await join(project.vscodePath, file.path);
								openExternalUrl(`${$editor}://file${absPath}`);
							}
							contextMenu.close();
						} catch {
							console.error('Failed to open in VSCode');
							toasts.error('Failed to open in VSCode');
						}
					}}
				/>
			{/if}
		</ContextMenuSection>
	{/snippet}
</ContextMenu>

<Modal
	width="small"
	title="Discard changes"
	bind:this={confirmationModal}
	onSubmit={confirmDiscard}
>
	{#snippet children(item)}
		<div>
			Discarding changes to the following files:
			<ul class="file-list">
				{#each item.files as file}
					<li><code class="code-string">{file.path}</code></li>
				{/each}
			</ul>
		</div>
	{/snippet}
	{#snippet controls(close, item)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" onclick={confirmDiscard(item)}>Confirm</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.file-list {
		list-style: disc;
		padding-left: 20px;
		padding-top: 6px;
	}
	.file-list li {
		padding: 2px;
	}
</style>
