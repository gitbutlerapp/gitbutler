<script lang="ts">
	import { Markdown } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	interface Props {
		content?: string;
		extraContent?: Snippet;
	}

	let { content }: Props = $props();
</script>

<div class="message-assistant text-13 text-body">
	{#if content}
		<Markdown {content} />
	{/if}
</div>

<style lang="postcss">
	.message-assistant {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: var(--message-max-width);
		padding: 8px 0;
		overflow: hidden;
		gap: 10px;
		text-wrap: wrap;
		word-break: break-word;
	}
	:global(.message-assistant .markdown > *) {
		/** With padding block quote background can still be full width. */
		padding-right: 32px;
	}
</style>
