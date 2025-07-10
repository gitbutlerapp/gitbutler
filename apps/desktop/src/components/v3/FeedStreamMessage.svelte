<script lang="ts">
	import { Feed, type InProgressAssistantMessage } from '$lib/feed/feed';
	import { getContext } from '@gitbutler/shared/context';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';

	type Props = {
		message: InProgressAssistantMessage;
	};

	const { message }: Props = $props();

	const feed = getContext(Feed);
	let messageContent = $state(message.content);
	const messageContentLines = $derived(messageContent.split('\n'));
	let bottom = $state<HTMLDivElement>();

	$effect(() => {
		const unsubscribe = feed.subscribeToMessage(message.id, (updatedMessage) => {
			messageContent += updatedMessage;
			if (bottom) {
				bottom.scrollIntoView({ behavior: 'smooth', block: 'end' });
			}
		});

		return () => {
			unsubscribe();
		};
	});
</script>

{#if messageContent === ''}
	<p class="thinking">Thinking...</p>
{:else}
	{#each messageContentLines as line, index (index)}
		{#if line === ''}
			<br />
		{:else}
			<Markdown content={line} />
		{/if}
	{/each}
{/if}
<div bind:this={bottom} style="margin-top: 8px;height: 1px; width: 100%;"></div>

<style>
	.thinking {
		color: var(--clr-text-3);
		font-style: italic;
		animation: pulse 1.5s ease-in-out infinite;
	}

	@keyframes pulse {
		0% {
			opacity: 1;
		}
		50% {
			opacity: 0.4;
		}
		100% {
			opacity: 1;
		}
	}
</style>
