<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import { CHAT_CHANNELS_SERVICE } from '@gitbutler/shared/chat/chatChannelsService';

	import { Button } from '@gitbutler/ui';
	import type { ChatMessage } from '@gitbutler/shared/chat/types';

	interface Props {
		projectId: string;
		changeId?: string;
		message: ChatMessage;
	}

	const { message, projectId, changeId }: Props = $props();

	const chatChannelService = inject(CHAT_CHANNELS_SERVICE);

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
