<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import HunkDiffBody from '@gitbutler/ui/hunkDiff/HunkDiffBody.svelte';
	import type { ContentSection } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		diffPath: string;
		content: ContentSection[];
		onGoToDiff: () => void;
	}

	const { diffPath, content, onGoToDiff }: Props = $props();
</script>

<div class="chat-message__diff-section">
	<div class="chat-message__diff-section__header">
		<div class="chat-message__diff-section__title">
			<FileIcon fileName={diffPath} size={16} />
			<p title={diffPath} class="text-12 text-body file-name">{diffPath}</p>
		</div>

		<div class="chat-message__diff-section__actions">
			<Button kind="ghost" size="icon" icon="show-in-code" onclick={onGoToDiff} />
		</div>
	</div>

	<div class="chat-message__diff-content">
		<table class="table__section">
			<HunkDiffBody filePath={diffPath} {content} />
		</table>
	</div>
</div>

<style lang="postcss">
	.chat-message__diff-section {
		display: flex;
		flex-direction: column;
		padding: 6px;
		gap: 8px;
		align-self: stretch;

		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.chat-message__diff-section__header {
		display: flex;
		align-items: center;
		align-self: stretch;
		justify-content: space-between;
	}

	.chat-message__diff-section__title {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.chat-message__diff-section:hover .chat-message__diff-section__actions {
		opacity: 1;
		pointer-events: auto;
	}

	.chat-message__diff-section__actions {
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--transition-fast);
	}

	.chat-message__diff-content {
		overflow: hidden;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-diff-count-border);
	}

	table,
	.table__section {
		width: 100%;
		border-collapse: separate;
		border-spacing: 0;
	}
</style>
