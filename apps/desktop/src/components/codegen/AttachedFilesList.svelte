<script lang="ts">
	import { FileIcon, Icon } from '@gitbutler/ui';
	import { fly } from 'svelte/transition';
	import type { AttachedFile } from '$lib/codegen/attachments.svelte';
	import type { FileAttachment } from '$lib/codegen/types';

	// Generic file type that works with both AttachedFile and FileAttachment
	export type DisplayFile = AttachedFile | FileAttachment;

	type Props = {
		attachedFiles: DisplayFile[];
		onRemoveFile?: (fileId: string) => void;
		showRemoveButton?: boolean;
	};

	const { attachedFiles, onRemoveFile, showRemoveButton = true }: Props = $props();

	function handleRemove(fileId: string): void {
		onRemoveFile?.(fileId);
	}

	function getFileName(file: DisplayFile): string {
		return 'file' in file ? file.file.name : file.name;
	}

	function getFilePreview(file: DisplayFile): string | undefined {
		return 'preview' in file ? file.preview : undefined;
	}
</script>

<div class="attached-files">
	{#each attachedFiles as attachedFile (attachedFile.id)}
		<div class="file-item" in:fly={{ y: 10, duration: 150 }}>
			<div class="file-content">
				{#if getFilePreview(attachedFile)}
					<img
						src={getFilePreview(attachedFile)}
						alt={getFileName(attachedFile)}
						class="file-preview"
					/>
				{:else}
					<FileIcon fileName={getFileName(attachedFile)} />
				{/if}

				<span class="text-12 text-semibold file-name" title={getFileName(attachedFile)}>
					{getFileName(attachedFile)}
				</span>
			</div>

			{#if showRemoveButton}
				<button
					type="button"
					class="remove-button"
					onclick={() => handleRemove(attachedFile.id)}
					aria-label="Remove {getFileName(attachedFile)}"
					title="Remove file"
				>
					<Icon name="cross-small" />
				</button>
			{/if}
		</div>
	{/each}
</div>

<style lang="postcss">
	.attached-files {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.file-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: var(--size-button);
		padding-right: 2px;
		padding-left: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--size-button);
	}

	.file-content {
		display: flex;
		flex: 1;
		align-items: center;
		margin-right: 8px;
		gap: 6px;
	}

	.file-preview {
		width: 20px;
		height: 20px;
		margin-left: -2px;
		object-fit: cover;
		border-radius: 20px;
		background-color: var(--clr-bg-3);
	}

	.file-name {
		max-width: 400px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.remove-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-button);
		height: var(--size-button);
		margin-left: -8px;
		color: var(--clr-text-3);
		transition: all 0.2s ease;

		&:hover {
			background-color: var(--clr-bg-error);
			color: var(--clr-text-error);
		}
	}
</style>
