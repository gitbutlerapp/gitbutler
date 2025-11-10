<script lang="ts">
	import AttachmentList from '$components/codegen/AttachmentList.svelte';
	import { Markdown } from '@gitbutler/ui';
	import type { PromptAttachment } from '$lib/codegen/types';

	interface Props {
		content?: string;
		attachments?: PromptAttachment[];
	}

	let { content, attachments }: Props = $props();
</script>

<div class="message-user">
	<div class="text-13 text-body message-bubble">
		<Markdown {content} />

		{#if attachments && attachments.length > 0}
			<AttachmentList {attachments} showRemoveButton={false} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.message-user {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		width: 100%;
		padding: 8px 0 16px 48px;
		gap: 10px;
	}

	.message-bubble {
		display: flex;
		flex-direction: column;
		max-width: var(--message-max-width);
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
			max-height: 400px;
			margin: 0 -12px;
			padding: 0;
			padding: 6px 12px;
			overflow-y: auto;
			border: none;
			border-radius: 0;
			background-color: var(--clr-scale-ntrl-20);
			color: white;
		}
	}
</style>
