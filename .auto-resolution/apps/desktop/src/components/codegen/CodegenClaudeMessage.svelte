<script lang="ts">
	import CodegenMessage from '$components/codegen/CodegenMessage.svelte';
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type Message } from '$lib/codegen/messages';
	import { Button, Icon } from '@gitbutler/ui';

	type Props = {
		message: Message;
	};
	const { message }: Props = $props();

	let toolCallsExpanded = $state(false);

	const toolDisplayLimit = 3;

	const toolsToDisplay = $derived.by(() => {
		if (message.type !== 'claude') return [];

		const loadingTools = message.toolCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = message.toolCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, 3);
	});
</script>

{#if message.type === 'user'}
	<CodegenMessage
		content={message.message}
		avatarUrl="https://avatars.githubusercontent.com/u/70?v=4"
		side="right"
		bubble
	/>
{:else if message.type === 'claude'}
	<CodegenMessage content={message.message} side="left">
		{#snippet extraContent()}
			{#if message.toolCalls.length > 0}
				{#if toolCallsExpanded}
					<div class="message-content-expanded-calls text-13">
						<div class="flex gap-10 items-center">
							<Button
								kind="ghost"
								icon="chevron-down"
								size="tag"
								onclick={() => (toolCallsExpanded = false)}
							/>
							<p>{message.toolCalls.length} tool calls</p>
						</div>
						{#each message.toolCalls as toolCall}
							<CodegenToolCall {toolCall} />
						{/each}
					</div>
				{:else}
					<div class="message-content-collapsed-calls text-13">
						<Button
							kind="ghost"
							icon="chevron-right"
							size="tag"
							onclick={() => (toolCallsExpanded = true)}
						/>
						<p>{message.toolCalls.length} tools in</p>
						<div class="message-content-collapsed-calls-entries clr-text-2">
							{#each toolsToDisplay as toolCall, idx}
								{#if toolCallLoading(toolCall)}
									<Icon name="spinner" />
								{/if}
								<div>{toolCall.name}</div>
								{#if idx !== toolsToDisplay.length - 1}
									<div>•</div>
								{/if}
							{/each}
						</div>
						{#if message.toolCalls.length > toolDisplayLimit}
							<p class="clr-text-2">And +{message.toolCalls.length - toolDisplayLimit} more</p>
						{/if}
					</div>
				{/if}
			{/if}
		{/snippet}
	</CodegenMessage>
{/if}

<style lang="postcss">
	.message-content-collapsed-calls {
		display: flex;

		align-items: center;
		width: fit-content;
		padding: 8px;
		padding-right: 12px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.message-content-collapsed-calls-entries {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.message-content-expanded-calls {
		display: flex;
		flex-direction: column;

		padding: 8px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
