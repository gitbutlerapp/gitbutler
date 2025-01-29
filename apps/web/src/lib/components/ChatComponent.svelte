<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { subscribeToChatChannel } from '$lib/chat/subscribe';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import Event from '$lib/components/chat/Event.svelte';
	import blankChat from '$lib/images/blank-chat.svg?raw';
	import { PatchEventsService } from '@gitbutler/shared/branches/patchEventsService';
	import { getPatchEvents } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { ApiPatchEvent } from '@gitbutler/shared/branches/types';

	interface Props {
		branchUuid: string;
		projectId: string;
		branchId: string;
		changeId: string;
		minimized: boolean;
		toggleMinimized: () => void;
	}

	const { projectId, changeId, branchId, branchUuid, minimized, toggleMinimized }: Props = $props();

	const authService = getContext(AuthService);
	const token = $derived(authService.token);
	const appState = getContext(AppState);
	const patchEventsService = getContext(PatchEventsService);

	const patchEvents = $derived(getPatchEvents(appState, patchEventsService, projectId, changeId));
	let chatMessagesContainer = $state<HTMLDivElement>();

	const seenEventIds = new Set<string>();

	function scrollToBottom() {
		if (chatMessagesContainer) {
			chatMessagesContainer.scrollTop = chatMessagesContainer.scrollHeight;
		}
	}

	async function onEvent(event: ApiPatchEvent) {
		if (seenEventIds.has(event.uuid)) return;
		seenEventIds.add(event.uuid);
		await patchEventsService.refreshPatchEvents(projectId, changeId);
		scrollToBottom();
	}

	$effect(() => {
		const unsubscribe = subscribeToChatChannel({
			token: $token ?? '',
			projectId,
			changeId,
			onEvent
		});

		return () => {
			unsubscribe();
			seenEventIds.clear();
		};
	});
</script>

{#if minimized}
	<div class="chat-card minimized">
		<Button style="pop" kind="ghost" onclick={toggleMinimized}>
			<Icon name="chat" />
		</Button>
	</div>
{:else}
	<div class="chat-card">
		<div class="chat-header">
			<Button size="tag" kind="ghost" onclick={toggleMinimized}>
				<Icon name="minus-small" />
			</Button>
		</div>
		<div class="chat-messages" bind:this={chatMessagesContainer}>
			<Loading loadable={patchEvents.current}>
				{#snippet children(patchEvents)}
					{#if patchEvents.events.length > 0}
						{#each patchEvents.events as event}
							<Event {projectId} {changeId} {event} />
						{/each}
					{:else}
						<div class="blank-state">
							<div class="blank-state-content">
								{@html blankChat}
								<div class="blank-message">
									<div class="blank-message-title">Give some feedback!</div>
									<p class="blank-message-text">
										If you're here, you must be important. This patch can use your help. Leave a
										comment or ask a question. Does this look right to you? How can it be improved?
										Is it perfect? Just let us know!
									</p>
								</div>
							</div>
						</div>
					{/if}
				{/snippet}
			</Loading>
		</div>
		<ChatInput {branchUuid} {projectId} {branchId} {changeId} />
	</div>
{/if}

<style lang="postcss">
	.chat-card {
		width: 100%;
		height: 50vh;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;

		border-radius: var(--radius-ml, 10px);
		border: 1px solid var(--clr-border-2, #d4d0ce);
		background: var(--clr-bg-1, #fff);

		&.minimized {
			padding: 8px 4px;
			height: fit-content;
			width: fit-content;
		}
	}

	.chat-header {
		display: flex;
		justify-content: flex-end;
		padding: 10px;
		border-bottom: 1px solid var(--clr-border-2, #d4d0ce);
	}

	.chat-messages {
		display: flex;
		flex-direction: column-reverse;
		overflow-y: scroll;
		scrollbar-width: none;
		&::-webkit-scrollbar {
			display: none;
		}
	}

	.blank-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-start;
		padding-left: 30px;
		margin-top: 20px;
	}

	.blank-state-content {
		display: flex;
		flex-direction: column;
		align-items: left;
		gap: 1rem;
		text-align: left;
		padding-left: 40px;
	}

	.blank-message {
		padding-left: 17px;
	}

	.blank-message-title {
		font-size: 1.3rem;
		font-weight: 600;
		color: var(--clr-text-2, #333);
		margin-top: 10px;
	}

	.blank-message-text {
		font-size: 0.9rem;
		color: var(--clr-text-2, #777);
		margin-top: 10px;
		width: 80%;
	}
</style>
