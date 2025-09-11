<script lang="ts">
	import { Markdown, ButPcAvatar } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	interface Props {
		content?: string;
		extraContent?: Snippet;
	}

	let { content, extraContent }: Props = $props();
</script>

<div class="message-assistant">
	<ButPcAvatar />

	<div class="text-13 text-body message-content">
		{#if content}
			<div class="message-bubble">
				<Markdown {content} />
			</div>
		{/if}

		{#if extraContent}
			{@render extraContent()}
		{/if}
	</div>
</div>

<style lang="postcss">
	.message-assistant {
		display: flex;
		align-items: flex-end;
		width: 100%;
		padding: 8px 0 16px;
		gap: 14px;
	}

	.message-content {
		display: flex;
		flex-direction: column;
		max-width: var(--message-max-width);
		overflow: hidden;
		gap: 10px;
		text-wrap: wrap;
		word-break: break-word;
	}

	.message-bubble {
		display: flex;
		flex-direction: column;
		width: fit-content;
		max-width: 100%;
		padding: 10px 14px;
		overflow: hidden;
		gap: 16px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-ml);
		text-wrap: wrap;
	}
</style>
