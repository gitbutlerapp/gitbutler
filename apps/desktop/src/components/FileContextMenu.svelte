<script lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import { Project } from '$lib/backend/projects';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import * as toasts from '$lib/utils/toasts';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
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
		trigger?: HTMLElement;
		isBinary?: boolean;
	}

	const { branchId, trigger, isUnapplied, isBinary = false }: Props = $props();

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	let confirmationModal: ReturnType<typeof Modal> | undefined;
	let contextMenu: ReturnType<typeof ContextMenu>;

	function isDeleted(item: any): boolean {
		if (!item.files || !Array.isArray(item.files)) return false;

		return item.files.some((f: unknown) => {
			if (!isAnyFile(f)) return false;
			return computeFileStatus(f) === 'D';
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

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.files && item.files.length > 0}
				{@const files = item.files}
				{#if files[0] instanceof LocalFile && !isUnapplied && !isBinary}
					<ContextMenuItem
						label="Discard changes"
						onclick={() => {
							confirmationModal?.show(item);
							contextMenu.close();
						}}
					/>
				{/if}
				{#if files.length === 1}
					<ContextMenuItem
						label="Copy Path"
						onclick={async () => {
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
						onclick={() => {
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
					onclick={async () => {
						try {
							if (!project) return;
							for (let file of item.files) {
								const path = getEditorUri({
									schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
									path: [project.vscodePath, file.path]
								});
								openExternalUrl(path);
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
				{/each}
			</ul>
		{:else}
			Discard the changes to all <span class="text-bold">
				{item.files.length} files
			</span>?
		{/if}
	{/snippet}
	{#snippet controls(close, item)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" onclick={() => confirmDiscard(item)}>Confirm</Button>
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
		background-color: var(--clr-bg-2);
		margin-top: 12px;
	}
</style>
