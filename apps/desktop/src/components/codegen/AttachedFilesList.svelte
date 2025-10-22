<script lang="ts">
	import { FileIcon, Icon } from '@gitbutler/ui';
	import { fly } from 'svelte/transition';
	import type { PersistedAttachment } from '$lib/codegen/types';

	// Generic file type that works with File objects and PersistedAttachment
	export type DisplayFile = File | PersistedAttachment;

	type Props = {
		attachedFiles: DisplayFile[];
		onRemoveFile?: (file: File) => void;
		showRemoveButton?: boolean;
	};

	const { attachedFiles, onRemoveFile, showRemoveButton = true }: Props = $props();

	function handleRemove(file: File): void {
		onRemoveFile?.(file);
	}

	function getFileName(file: DisplayFile): string {
		if (file instanceof File) {
			return file.name;
		} else if (file.type === 'file') {
			return file.subject.name;
		}
		return '';
	}

	async function getFilePreview(file: DisplayFile): Promise<string | undefined> {
		// Only File objects can have previews, PersistedAttachment cannot
		if (file instanceof File) {
			return await generatePreview(file);
		}
		return undefined;
	}

	async function generatePreview(file: File): Promise<string | undefined> {
		if (file.type.startsWith('image/')) {
			return new Promise((resolve) => {
				const reader = new FileReader();
				reader.onload = (e) => resolve(e.target?.result as string);
				reader.onerror = () => resolve(undefined);
				reader.readAsDataURL(file);
			});
		}
		return undefined;
	}
</script>

<div class="attached-files">
	{#each attachedFiles as attachedFile}
		<div class="file-item" in:fly={{ y: 10, duration: 150 }}>
			<div class="file-content">
				{#await getFilePreview(attachedFile) then preview}
					{#if preview}
						<img src={preview} alt={getFileName(attachedFile)} class="file-preview" />
					{:else}
						<FileIcon fileName={getFileName(attachedFile)} />
					{/if}
				{/await}

				<span class="text-12 text-semibold file-name" title={getFileName(attachedFile)}>
					{getFileName(attachedFile)}
				</span>
			</div>

			{#if showRemoveButton && attachedFile instanceof File}
				<button
					type="button"
					class="remove-button"
					onclick={() => handleRemove(attachedFile)}
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
