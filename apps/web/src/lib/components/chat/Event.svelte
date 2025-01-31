<script lang="ts">
	import IssueUpdate from './IssueUpdate.svelte';
	import Message from './Message.svelte';
	import type { PatchEvent } from '@gitbutler/shared/branches/types';

	interface Props {
		projectId: string;
		changeId: string;
		event: PatchEvent;
	}

	const { event, projectId, changeId }: Props = $props();
</script>

{#if event.eventType === 'chat' && event.object}
	<Message {projectId} {changeId} message={event.object} />
{:else if event.eventType === 'issue_status'}
	<IssueUpdate issueUpdate={event.object} />
{/if}
