<script lang="ts">
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { ChatMessage } from '@gitbutler/shared/chat/types';

	interface Props {
		projectId: string;
		changeId?: string;
		message: ChatMessage;
	}

	const { message, projectId, changeId }: Props = $props();

	const chatChannelService = getContext(ChatChannelsService);

	let isResolving = $state<boolean>(false);

	async function resolveIssue() {
		if (isResolving) return;
		isResolving = true;
		try {
			await chatChannelService.patchChatMessage({
				projectId,
				changeId,
				messageUuid: message.uuid,
				resolved: true
			});
		} finally {
			isResolving = false;
		}
	}
</script>

{#if message.issue && !message.resolved}
	<div class="chat-message-actions">
		<Button
			style="neutral"
			kind="outline"
			icon="tick-small"
			loading={isResolving}
			onclick={resolveIssue}>Resolve issue</Button
		>
	</div>
{/if}
