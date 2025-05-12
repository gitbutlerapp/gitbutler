<!-- TODO: Delete this file after V3 has shipped. -->
<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { LocalFile } from '$lib/files/file';
	import { isAnyFile } from '$lib/files/file';
	import { Project } from '$lib/project/project';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import { join } from '@tauri-apps/api/path';
	import type { Writable } from 'svelte/store';

	interface Props {
		isUnapplied: boolean;
		projectId: string;
		stackId?: string;
		trigger?: HTMLElement;
		isBinary?: boolean;
	}

	const { projectId, stackId, trigger, isUnapplied, isBinary = false }: Props = $props();

	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const stackService = getContext(StackService);

	let confirmationModal: ReturnType<typeof Modal<{ files: LocalFile[] }>> | undefined;
	let contextMenu: ReturnType<typeof ContextMenu>;

	function isDeleted(item: any): boolean {
		if (!item.files || !Array.isArray(item.files)) return false;

		return item.files.some((f: unknown) => {
			if (!isAnyFile(f)) return false;
			return computeFileStatus(f) === 'D';
		});
	}

	async function confirmDiscard(item: any) {
		if (!stackId) {
			console.error('Stack ID is not set');
			toasts.error('Failed to discard changes');
			return;
		}
		await stackService.legacyUnapplyFiles({ projectId, stackId, files: item.files });
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
							if (!project) return;
							const absPath = await join(project.path, item.files[0].path);
							await writeClipboard(absPath, {
								errorMessage: 'Failed to copy path'
							});
							contextMenu.close();
						}}
					/>
					<ContextMenuItem
						label="Copy Relative Path"
						onclick={async () => {
							if (!project) return;
							await writeClipboard(item.files[0].path, {
								errorMessage: 'Failed to copy relative path'
							});
							contextMenu.close();
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
		<AsyncButton style="error" type="submit" action={async () => await confirmDiscard(item)}>
			Confirm
		</AsyncButton>
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
