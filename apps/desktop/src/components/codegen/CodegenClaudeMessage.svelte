<script lang="ts">
	import CodegenAssistantMessage from '$components/codegen/CodegenAssistantMessage.svelte';
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import CodegenToolCalls from '$components/codegen/CodegenToolCalls.svelte';
	import CodegenUserMessage from '$components/codegen/CodegenUserMessage.svelte';
	import { type Message } from '$lib/codegen/messages';

	type Props = {
		message: Message;
		onApproval?: (id: string) => Promise<void>;
		onRejection?: (id: string) => Promise<void>;
		userAvatarUrl?: string;
	};
	const { message, onApproval, onRejection, userAvatarUrl }: Props = $props();
</script>

{#if message.type === 'user'}
	<CodegenUserMessage content={message.message} avatarUrl={userAvatarUrl} />
{:else if message.type === 'claude'}
	<CodegenAssistantMessage content={message.message}>
		{#snippet extraContent()}
			<CodegenToolCalls toolCalls={message.toolCalls} />

			{#if message.toolCallsPendingApproval.length > 0}
				{#each message.toolCallsPendingApproval as toolCall}
					<CodegenToolCall
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
