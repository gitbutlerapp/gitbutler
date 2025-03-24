<!-- This is a V3 replacement for `FileContextMenu.svelte` -->
<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { isTreeChange, type TreeChange } from '$lib/hunks/change';
	import { Project } from '$lib/project/project';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItemV3.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import { join } from '@tauri-apps/api/path';
	import type { DiffSpec } from '$lib/hunks/hunk';
	import type { Writable } from 'svelte/store';

	type Props = {
		isUnapplied: boolean;
		branchId?: string;
		trigger?: HTMLElement;
		isBinary?: boolean;
	};

	type FileItem = {
		changes: TreeChange[];
	};

	function isFileItem(item: unknown): item is FileItem {
		return (
			typeof item === 'object' &&
			item !== null &&
			'changes' in item &&
			Array.isArray(item.changes) &&
			item.changes.every(isTreeChange)
		);
	}

	const { trigger, isUnapplied, isBinary = false }: Props = $props();
	const [stackService, project] = inject(StackService, Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const [discardChanges] = stackService.discardChanges();
	let confirmationModal: ReturnType<typeof Modal> | undefined;
	let contextMenu: ReturnType<typeof ContextMenu>;
	const projectId = $derived(project.id);

	function isDeleted(item: FileItem): boolean {
		return item.changes.some((change) => {
			return change.status.type === 'Deletion';
		});
	}

	function confirmDiscard(item: FileItem) {
		const worktreeChanges: DiffSpec[] = item.changes.map((change) => ({
			previousPathBytes: null,
			pathBytes: change.path,
			hunkHeaders: []
		}));

		discardChanges({
			projectId,
			worktreeChanges
		});

		close();
	}

	export function open(e: MouseEvent, item: FileItem) {
		contextMenu.open(e, item);
	}
</script>

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
	{#snippet children(item: unknown)}
		{#if isFileItem(item)}
			{@const deletion = isDeleted(item)}
			<ContextMenuSection>
				{#if item.changes.length > 0}
					{@const changes = item.changes}
					{#if !isUnapplied && !isBinary}
						<ContextMenuItem
							label="Discard changes"
							onclick={() => {
								confirmationModal?.show(item);
								contextMenu.close();
							}}
						/>
					{/if}
					{#if changes.length === 1}
						<ContextMenuItem
							label="Copy Path"
							onclick={async () => {
								if (!project) return;
								const absPath = await join(project.path, changes[0]!.path);
								await writeClipboard(absPath, 'Failed to copy path');
								contextMenu.close();
								// dismiss();
							}}
						/>
						<ContextMenuItem
							label="Copy Relative Path"
							onclick={async () => {
								if (!project) return;
								await writeClipboard(changes[0]!.path, 'Failed to copy relative path');
								contextMenu.close();
							}}
						/>
					{/if}
					<ContextMenuItem
						label="Open in {$userSettings.defaultCodeEditor.displayName}"
						disabled={deletion}
						onclick={async () => {
							try {
								if (!project) return;
								for (let change of changes) {
									const path = getEditorUri({
										schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
										path: [project.vscodePath, change.path]
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
		{:else}
			<ContextMenuSection>
				<p class="text-13">
					{'Woops! Malformed data :('}
				</p>
			</ContextMenuSection>
		{/if}
	{/snippet}
</ContextMenu>

<Modal
	width="small"
	type="warning"
	title="Discard changes"
	bind:this={confirmationModal}
	onSubmit={(_, item) => isFileItem(item) && confirmDiscard(item)}
>
	{#snippet children(item)}
		{#if isFileItem(item)}
			{@const changes = item.changes}
			{#if changes.length < 10}
				<p class="discard-caption">
					Are you sure you want to discard the changes<br />to the following files:
				</p>
				<ul class="file-list">
					{#each changes as change}
						<FileListItem
							filePath={change.path}
							fileStatus={computeChangeStatus(change)}
							clickable={false}
							listMode="list"
						/>
					{/each}
				</ul>
			{:else}
				Discard the changes to all <span class="text-bold">
					{changes.length} files
				</span>?
			{/if}
		{:else}
			<p class="text-13">
				{'Woops! Malformed data :('}
			</p>
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
