<script lang="ts">
	import CodegenAssistantMessage from '$components/codegen/CodegenAssistantMessage.svelte';
	import CodegenGitButlerMessage from '$components/codegen/CodegenGitButlerMessage.svelte';
	import CodegenServiceMessage from '$components/codegen/CodegenServiceMessage.svelte';
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import CodegenToolCalls from '$components/codegen/CodegenToolCalls.svelte';
	import CodegenUserMessage from '$components/codegen/CodegenUserMessage.svelte';
	import { type Message } from '$lib/codegen/messages';
	import { Icon, Markdown } from '@gitbutler/ui';
	import type { PermissionDecision } from '$lib/codegen/types';

	type Props = {
		projectId: string;
		message: Message;
		onPermissionDecision?: (
			id: string,
			decision: PermissionDecision,
			useWildcard: boolean
		) => Promise<void>;
		toolCallsExpandedState?: Map<string, boolean>;
	};
	const { projectId, message, onPermissionDecision, toolCallsExpandedState }: Props = $props();

	let expanded = $state(false);
</script>

{#if message.source === 'user'}
	<CodegenUserMessage content={message.message} attachments={message.attachments} />
{:else if message.source === 'claude'}
	{#if 'subtype' in message && message.subtype === 'compaction'}
		<CodegenServiceMessage style="neutral" face="compacted" reverseElementsOrder>
			{#snippet extraContent()}
				{@render compactionSummary(message.message)}
			{/snippet}
		</CodegenServiceMessage>
	{:else}
		<CodegenAssistantMessage content={message.message} />
		<CodegenToolCalls
			{projectId}
			toolCalls={message.toolCalls}
			messageId={message.createdAt}
			{toolCallsExpandedState}
		/>
		{#if message.toolCallsPendingApproval.length > 0}
			{#each message.toolCallsPendingApproval as toolCall}
				<CodegenToolCall
					{projectId}
					style="standalone"
					{toolCall}
					requiresApproval={{
						onPermissionDecision: async (id, decision, useWildcard) =>
							await onPermissionDecision?.(id, decision, useWildcard)
					}}
				/>
			{/each}
		{/if}
	{/if}
{:else if message.source === 'gitButler'}
	<CodegenGitButlerMessage {projectId} {message} />
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
				<p class="text-13 text-italic clr-text-2 truncate">
					Conversation compacted to preserve context
				</p>
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
		max-width: 100%;
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

	.message-assistant-wrapper {
		display: flex;
		flex-direction: column;
		max-width: var(--message-max-width);
	}

	.timestamp {
		color: var(--clr-text-2);
		transition: color var(--transition-medium);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
