<script lang="ts">
	import { Markdown, Button } from '@gitbutler/ui';
	import type { FileAttachment } from '$lib/codegen/types';

	interface Props {
		content?: string;
		attachments?: FileAttachment[];
	}

	let { content, attachments }: Props = $props();

	function downloadFile(attachment: FileAttachment) {
		try {
			// Decode the base64 content
			const binaryData = atob(attachment.content);
			const bytes = new Uint8Array(binaryData.length);
			for (let i = 0; i < binaryData.length; i++) {
				bytes[i] = binaryData.charCodeAt(i);
			}

			// Create blob and download link
			const blob = new Blob([bytes], { type: attachment.mimeType });
			const url = URL.createObjectURL(blob);

			const link = document.createElement('a');
			link.href = url;
			link.download = attachment.name;
			document.body.appendChild(link);
			link.click();
			document.body.removeChild(link);
			URL.revokeObjectURL(url);
		} catch (error) {
			console.error('Failed to download file:', error);
		}
	}

	function formatFileSize(bytes: number): string {
		if (bytes === 0) return '0 Bytes';
		const k = 1024;
		const sizes = ['Bytes', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
	}
</script>

<div class="message-user">
	<div class="text-13 text-body message-bubble">
		<Markdown {content} />
		{#if attachments && attachments.length > 0}
			<div class="attachments">
				<div class="text-12 text-muted">Attachments:</div>
				{#each attachments as attachment}
					<div class="attachment">
						<Button
							kind="outline"
							icon="attachment"
							onclick={() => downloadFile(attachment)}
							class="attachment-button"
						>
							<span class="attachment-name">{attachment.name}</span>
							<span class="attachment-size">({formatFileSize(attachment.size)})</span>
						</Button>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.message-user {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		width: 100%;
		padding: 8px 0 16px;
		gap: 10px;
	}

	.message-bubble {
		display: flex;
		flex-direction: column;
		max-width: calc(var(--message-max-width) - 6%);
		padding: 10px 14px;
		overflow: hidden;
		gap: 16px;
		border-radius: var(--radius-ml);
		border-bottom-right-radius: 0;
		background-color: var(--clr-bg-2);
		text-wrap: wrap;
		word-break: break-word;

		/* make code blocks visible */
		:global(.markdown pre) {
			background-color: var(--clr-bg-1);
		}
	}

	.attachments {
		margin-top: 12px;
		padding-top: 8px;
		border-top: 1px solid var(--clr-border-2);
	}

	.attachment {
		margin-top: 4px;
	}

	:global(.attachment-button) {
		justify-content: flex-start !important;
		width: 100%;
		text-align: left !important;
	}

	.attachment-name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.attachment-size {
		margin-left: 8px;
		opacity: 0.7;
	}
</style>
