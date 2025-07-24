<script lang="ts">
	import FeedItemKind from '$components/FeedItemKind.svelte';
	import { FEED_FACTORY, type InProgressAssistantMessage } from '$lib/feed/feed';
	import { inject } from '@gitbutler/shared/context';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import type { ToolCall } from '$lib/ai/tool';

	type Props = {
		projectId: string;
		message: InProgressAssistantMessage;
	};

	const { projectId, message }: Props = $props();

	const feedFactory = inject(FEED_FACTORY);
	const feed = $derived(feedFactory.getFeed(projectId));
	let toolCalls = $state<ToolCall[]>(message.toolCalls);
	let messageContent = $state(message.content);
	const messageContentLines = $derived(messageContent.split('\n'));

	let bottom = $state<HTMLDivElement>();

	function handleToken(token: string) {
		messageContent += token;
		if (bottom) {
			bottom.scrollIntoView({ behavior: 'instant', block: 'end' });
		}
	}

	function handleToolCall(toolCall: ToolCall) {
		toolCalls.push(toolCall);
		if (bottom) {
			bottom.scrollIntoView({ behavior: 'instant', block: 'end' });
		}
	}

	$effect(() => {
		const unsubscribe = feed.subscribeToMessage(message.id, (updatedMessage) => {
			switch (updatedMessage.type) {
				case 'token':
					return handleToken(updatedMessage.token);
				case 'tool-call':
					return handleToolCall(updatedMessage.toolCall);
			}
		});

		return () => {
			unsubscribe();
		};
	});
</script>

<div>
	{#if messageContent === '' && toolCalls.length === 0}
		<p class="thinking">Thinking...</p>
	{:else}
		{#if toolCalls.length > 0}
			<p class="vibing">Vibing</p>
			{#each toolCalls as toolCall, index (index)}
				<FeedItemKind type="tool-call" {projectId} {toolCall} />
			{/each}
		{/if}

		{#each messageContentLines as line, index (index)}
			{#if line === ''}
				<br />
			{:else}
				<Markdown content={line} />
			{/if}
		{/each}
	{/if}
	<div bind:this={bottom} style="margin-top: 8px; height: 1px; width: 100%;"></div>
</div>

<style>
	.thinking,
	.vibing {
		margin: 4px 0;
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
