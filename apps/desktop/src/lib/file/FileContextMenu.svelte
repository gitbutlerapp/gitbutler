<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { isAnyFile, LocalFile } from '$lib/vbranches/types';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import { join } from '@tauri-apps/api/path';
	import type { Writable } from 'svelte/store';

	interface Props {
		isUnapplied: boolean;
		branchId?: string;
		target?: HTMLElement;
		isBinary?: boolean;
	}

	const { branchId, target, isUnapplied, isBinary = false }: Props = $props();

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

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
				{#if files[0] instanceof LocalFile && !isUnapplied && !isBinary}
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
					label="Open in {$userSettings.defaultCodeEditor.displayName}"
					disabled={isDeleted(item)}
					on:click={async () => {
						try {
							if (!project) return;
							for (let file of item.files) {
								const absPath = await join(project.vscodePath, file.path);
								openExternalUrl(
									`${$userSettings.defaultCodeEditor.schemeIdentifer}://file${absPath}`
								);
							}
							contextMenu.close();
						} catch {
							console.error('Failed to open in editor');
							toasts.error('Failed to open in editor');
						}
					}}
				/>
			{/if}
		</ContextMenuSection>
	{/snippet}
</ContextMenu>

<Modal
	width="small"
	type="warning"
	title="Discard changes"
	bind:this={confirmationModal}
	onSubmit={confirmDiscard}
>
	{#snippet children(item)}
		{#if item.files.length < 10}
			<p class="discard-caption">
				Are you sure you want to discard the changes<br />to the following files:
			</p>
			<ul class="file-list">
				{#each item.files as file}
					<FileListItem filePath={file.path} fileStatus={file.status} clickable={false} />
					<!-- <li><code class="code-string">{file.path}</code></li> -->
				{/each}
			</ul>
		{:else}
			Discard the changes to all <span class="text-bold">
				{item.files.length} files
			</span>?
		{/if}
	{/snippet}
	{#snippet controls(close, item)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" onclick={confirmDiscard(item)}>Confirm</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.discard-caption {
		color: var(--clr-text-2);
	}
	.file-list {
		padding: 4px 0;
		border-radius: var(--radius-m);
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		margin-top: 12px;
	}
</style>
