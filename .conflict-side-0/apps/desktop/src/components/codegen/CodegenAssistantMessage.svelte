<script lang="ts">
	import { Markdown } from '@gitbutler/ui';

	interface Props {
		content?: string;
	}

	let { content }: Props = $props();
</script>

{#if content}
	<div class="message-assistant text-13 text-body">
		<Markdown {content} />
	</div>
{/if}

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
