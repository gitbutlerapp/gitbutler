<script lang="ts">
	import CodegenAssistantMessage from '$components/codegen/CodegenAssistantMessage.svelte';
	import CodegenServiceMessage from '$components/codegen/CodegenServiceMessage.svelte';
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import CodegenToolCalls from '$components/codegen/CodegenToolCalls.svelte';
	import CodegenUserMessage from '$components/codegen/CodegenUserMessage.svelte';
	import { type Message } from '$lib/codegen/messages';
	import { Icon, Markdown } from '@gitbutler/ui';

	type Props = {
		message: Message;
		onApproval?: (id: string) => Promise<void>;
		onRejection?: (id: string) => Promise<void>;
	};
	const { message, onApproval, onRejection }: Props = $props();

	let expanded = $state(false);
</script>

{#if message.type === 'user'}
	<CodegenUserMessage content={message.message} />
{:else if message.type === 'claude'}
	{#if 'subtype' in message && message.subtype === 'compaction'}
		<CodegenServiceMessage style="neutral" face="compacted" reverseElementsOrder>
			{#snippet extraContent()}
				{@render compactionSummary(message.message)}
			{/snippet}
		</CodegenServiceMessage>
	{:else}
		<CodegenAssistantMessage content={message.message}>
			{#snippet extraContent()}
				<CodegenToolCalls toolCalls={message.toolCalls} />

				{#if message.toolCallsPendingApproval.length > 0}
					{#each message.toolCallsPendingApproval as toolCall}
						<CodegenToolCall
							style="standalone"
							{toolCall}
							requiresApproval={{
								onApproval: async (id) => await onApproval?.(id),
								onRejection: async (id) => await onRejection?.(id)
							}}
						/>
					{/each}
				{/if}
			{/snippet}
		</CodegenAssistantMessage>
	{/if}
{/if}

{#snippet compactionSummary(summary: string)}
	<div class="compaction-summary__wrapper">
		<div class="compaction-summary" class:expanded>
			<button
				type="button"
				class="compaction-summary__header"
				onclick={() => (expanded = !expanded)}
			>
				<div class="compaction-summary__arrow" class:expanded>
					<Icon name="chevron-right" />
				</div>
				<p class="text-13 text-italic clr-text-2">Conversation compacted to preserve context</p>
			</button>
			{#if expanded}
				<div class="text-13 compaction-summary__content">
					<Markdown content={summary} />
				</div>
			{/if}
		</div>
	</div>
{/snippet}

<style lang="postcss">
	.compaction-summary__wrapper {
		max-width: var(--message-max-width);
	}

	.compaction-summary {
		width: fit-content;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);

		&.expanded {
			width: 100%;
		}
	}

	.compaction-summary__header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 10px 12px 10px 8px;
		gap: 8px;
		background-color: var(--clr-bg-2);
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2-muted);

			.compaction-summary__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.compaction-summary__arrow {
		display: flex;
		color: var(--clr-text-3);
		transition: transform var(--transition-medium);

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.compaction-summary__content {
		padding: 12px;
	}
</style>
