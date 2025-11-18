<script lang="ts">
	import FeedItemKind from '$components/FeedItemKind.svelte';
	import { FEED_FACTORY, type InProgressAssistantMessage, type TodoState } from '$lib/feed/feed';
	import { inject } from '@gitbutler/core/context';
	import { Icon, Markdown } from '@gitbutler/ui';

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
	let todos = $state<TodoState[]>(message.todos);
	const paragraphs = $derived(messageContent.split('\n\n'));

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

	function handleTodoUpdate(list: TodoState[]) {
		todos = list;
	}

	$effect(() => {
		const unsubscribe = feed.subscribeToMessage(message.id, (updatedMessage) => {
			switch (updatedMessage.type) {
				case 'token':
					return handleToken(updatedMessage.token);
				case 'tool-call':
					return handleToolCall(updatedMessage.toolCall);
				case 'todo-update':
					return handleTodoUpdate(updatedMessage.list);
			}
		});

		return () => {
			unsubscribe();
		};
	});
</script>

{#snippet todoItem(todo: TodoState)}
	<div class="stream-message__todo-item">
		{#if todo.status === 'in-progress'}
			<Icon name="spinner" opacity={0.6} />
		{:else if todo.status === 'success'}
			<Icon name="success" color="success" opacity={0.6} />
		{:else if todo.status === 'failed'}
			<Icon name="error" color="error" opacity={0.6} />
		{:else if todo.status === 'waiting'}
			<Icon name="info" opacity={0.6} />
		{/if}
		<span class="text-12" class:suceeded={todo.status === 'success'}>{todo.title}</span>
	</div>
{/snippet}

{#snippet todoList()}
	<div class="stream-message__todo-list">
		{#each todos as todo (todo.id)}
			{@render todoItem(todo)}
		{/each}
	</div>
{/snippet}

<div class="stream-message text-14">
	{#if todos.length > 0}
		{@render todoList()}
	{/if}

	{#if messageContent === '' && toolCalls.length === 0}
		<p class="thinking">Thinking...</p>
	{:else}
		{#if toolCalls.length > 0}
			<p class="vibing">Vibing</p>
			<div class="stream-message__tool-calls">
				{#each toolCalls as toolCall, index (index)}
					<FeedItemKind type="tool-call" {toolCall} />
				{/each}
			</div>
		{/if}

		<div class="text-content">
			{#each paragraphs as paragraph, index (index)}
				<Markdown content={paragraph} />
			{/each}
		</div>
	{/if}
	<div bind:this={bottom} style="margin-top: 8px; height: 1px; width: 100%;"></div>
</div>

<style lang="postcss">
	.stream-message {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.stream-message__todo-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.stream-message__todo-item {
		display: flex;
		gap: 9px;

		& > span.suceeded {
			color: var(--clr-text-2);
			text-decoration: line-through;
		}
	}

	.stream-message__tool-calls {
		display: flex;
		flex-direction: column;
		padding: 10px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-ml);
	}

	.thinking,
	.vibing {
		margin: 4px 0;
		color: var(--clr-text-3);
		font-style: italic;
		animation: pulse 1.5s ease-in-out infinite;
	}

	.text-content {
		display: flex;
		flex-direction: column;
		gap: 8px;
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
