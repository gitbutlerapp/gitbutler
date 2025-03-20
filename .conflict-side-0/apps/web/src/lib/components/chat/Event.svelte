<script lang="ts">
	import IssueUpdate from './IssueUpdate.svelte';
	import Message from './Message.svelte';
	import PatchStatus from './PatchStatus.svelte';
	import PatchVersion from './PatchVersion.svelte';
	import type { ChatEvent, PatchEvent } from '@gitbutler/shared/patchEvents/types';

	interface Props {
		highlightedMessageUuid: string | undefined;
		projectId: string;
		changeId: string;
		event: PatchEvent;
		replyTo: (chatEvent: ChatEvent) => void;
		scrollToMessage: (uuid: string) => void;
	}

	const { event, projectId, changeId, highlightedMessageUuid, replyTo, scrollToMessage }: Props =
		$props();
</script>

{#if event.eventType === 'chat'}
	<Message
		{projectId}
		{changeId}
		{event}
		highlight={highlightedMessageUuid === event.object.uuid}
		onReply={() => replyTo(event)}
		{scrollToMessage}
	/>
{:else if event.eventType === 'issue_status'}
	<IssueUpdate {event} />
{:else if event.eventType === 'patch_version'}
	<PatchVersion {event} />
{:else if event.eventType === 'patch_status'}
	<PatchStatus {event} />
{/if}
