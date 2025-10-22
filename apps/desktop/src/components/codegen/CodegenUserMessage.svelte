<script lang="ts">
	import AttachedFilesList from '$components/codegen/AttachedFilesList.svelte';
	import { Markdown } from '@gitbutler/ui';
	import type { PersistedAttachment } from '$lib/codegen/types';

	interface Props {
		content?: string;
		attachments?: PersistedAttachment[];
	}

	let { content, attachments }: Props = $props();
</script>

<div class="message-user">
	<div class="text-13 text-body message-bubble">
		<Markdown {content} />

		{#if attachments && attachments.length > 0}
			<hr class="message-user__divider" />
			<AttachedFilesList attachedFiles={attachments} showRemoveButton={false} />
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
		gap: 10px;
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

	.message-user__divider {
		width: 100%;
		border: none;
		border-top: 1px dotted var(--clr-border-2);
	}
</style>
