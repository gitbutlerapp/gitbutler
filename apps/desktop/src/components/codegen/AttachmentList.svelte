<script lang="ts">
	import { FileIcon, Icon, Tooltip } from '@gitbutler/ui';
	import { splitFilePath } from '@gitbutler/ui/utils/filePath';
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

	function getTooltipText(attachment: PromptAttachment): string {
		switch (attachment.type) {
			case 'file': {
				const { commitId, path } = attachment;
				const commitInfo = commitId ? ` (from commit ${commitId})` : '';
				return `${path}${commitInfo}`;
			}
			case 'lines': {
				const { commitId, path, start, end } = attachment;
				const commitInfo = commitId ? ` (from commit ${commitId})` : '';
				return `Lines ${start}-${end}${path}${commitInfo}`;
			}
			case 'commit':
				return `${attachment.commitId}`;
			case 'directory':
				return `${attachment.path}`;
		}
		return '';
	}
</script>

<div class="attachments">
	{#each attachments as attachment}
		<div class="attachment" in:fly={{ y: 10, duration: 150 }}>
			<Tooltip text={getTooltipText(attachment)}>
				<div class="attachment-content text-12 text-semibold">
					<!-- COMMIT -->
					{#if attachment.type === 'commit'}
						<Icon name="commit" color="var(--clr-text-2)" />
						<span class="path">
							#{attachment.commitId.slice(0, 6)}
						</span>
					{/if}
					<!-- FILE -->
					{#if attachment.type === 'file'}
						{@const { path, commitId } = attachment}
						<FileIcon fileName={path} />

						<span class="path">
							{splitFilePath(path).filename}
						</span>

						{#if commitId}
							<Icon name="commit" color="var(--clr-text-3)" />
							<Tooltip text={commitId}>
								<span class="commit-badge">
									#{commitId.slice(0, 6)}
								</span>
							</Tooltip>
						{/if}
					{/if}
					<!-- LINES -->
					{#if attachment.type === 'lines'}
						{@const { commitId, path, start, end } = attachment}
						<FileIcon fileName={path} />

						<span class="path">
							{splitFilePath(path).filename}
						</span>

						<Icon name="text" color="var(--clr-text-3)" />
						<span>
							{start}:{end}
						</span>

						{#if commitId}
							<Icon name="commit" color="var(--clr-text-3)" />
							<Tooltip text={commitId}>
								<span class="commit-badge">
									#{commitId.slice(0, 6)}
								</span>
							</Tooltip>
						{/if}
					{/if}
					<!-- DIRECTORY -->
					{#if attachment.type === 'directory'}
						{@const { path } = attachment}
						<span class="path">
							{path}
						</span>
					{/if}
					<!-- FOLDER -->
					{#if attachment.type === 'folder'}
						{@const { path } = attachment}
						<Icon name="folder" color="var(--clr-text-2)" />
						<span class="path">
							{path}
						</span>
					{/if}
				</div>
			</Tooltip>

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

	.commit-badge {
		color: var(--clr-text-3);
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
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
