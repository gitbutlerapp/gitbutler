<script lang="ts">
	import { FileIcon, Icon } from '@gitbutler/ui';
	import { abbreviatePath } from '@gitbutler/ui/utils/filePath';
	import { fly } from 'svelte/transition';
	import type { PromptAttachment } from '$lib/codegen/types';

	type Props = {
		attachments: PromptAttachment[];
		showRemoveButton?: boolean;
		onRemove?: (attachment: PromptAttachment) => void;
	};

	const { attachments, onRemove: onRemoveFile, showRemoveButton = true }: Props = $props();

	function handleRemove(attachment: PromptAttachment): void {
		onRemoveFile?.(attachment);
	}
</script>

<div class="attachments">
	{#each attachments as attachment}
		<div class="attachment" in:fly={{ y: 10, duration: 150 }}>
			<div class="attachment-content text-12 text-semibold">
				{#if attachment.type === 'commit'}
					<span class="path" title={attachment.commitId}>
						#{attachment.commitId.slice(0, 6)}
					</span>
				{/if}
				{#if attachment.type === 'file'}
					<FileIcon fileName={attachment.path} />

					<span class="path" title={attachment.path}>
						{abbreviatePath(attachment.path)}
					</span>
				{/if}
				{#if attachment.type === 'hunk'}
					{@const { path, start, end } = attachment}
					<FileIcon fileName={attachment.path} />

					<span class="path" title={attachment.path}>
						{abbreviatePath(path)}
					</span>
					<span>
						{start}:{end}
					</span>
				{/if}
			</div>

			{#if showRemoveButton}
				<button
					type="button"
					class="remove-button"
					onclick={() => handleRemove(attachment)}
					aria-label="Remove {attachment}"
					title="Remove file"
				>
					<Icon name="cross-small" />
				</button>
			{/if}
		</div>
	{/each}
</div>

<style lang="postcss">
	.attachments {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.attachment {
		display: flex;
		flex-shrink: 1;
		align-items: center;
		justify-content: space-between;
		height: var(--size-button);
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--size-button);
		cursor: default;
	}

	.attachment-content {
		display: flex;
		flex: 1;
		align-items: center;
		padding: 0 8px;
		overflow-x: hidden;
		gap: 6px;
	}

	.path {
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
