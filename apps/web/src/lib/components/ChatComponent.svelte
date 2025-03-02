<script lang="ts">
	import ShowChatButton from '$lib/components/ShowChatButton.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import Event from '$lib/components/chat/Event.svelte';
	import { type DiffSelection } from '$lib/diff/lineSelection.svelte';
	import blankChat from '$lib/images/blank-chat.svg?raw';
	import { PatchEventsService } from '@gitbutler/shared/branches/patchEventsService';
	import { getPatchEvents } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		messageUuid: string | undefined;
		isPatchAuthor: boolean | undefined;
		branchUuid: string;
		projectId: string;
		branchId: string;
		changeId: string;
		minimized: boolean;
		isUserLoggedIn: boolean | undefined;
		isTabletMode: boolean;
		onMinimizeToggle: () => void;
		diffSelection: DiffSelection | undefined;
		clearDiffSelection: () => void;
	}

	let {
		messageUuid,
		projectId,
		changeId,
		branchId,
		branchUuid,
		minimized,
		isPatchAuthor,
		isUserLoggedIn,
		isTabletMode,
		onMinimizeToggle,
		diffSelection,
		clearDiffSelection
	}: Props = $props();

	const appState = getContext(AppState);
	const patchEventsService = getContext(PatchEventsService);

	const patchEvents = $derived(getPatchEvents(appState, patchEventsService, projectId, changeId));
	// This shouldn't be reactive as is just to check if the message was scrolled to already.
	// Only a hard reload should trigger the scroll again.
	const scrolledMessages = new Set<string>();

	function scrollToMessageWithDelay(uuid: string, delay: number) {
		if (scrolledMessages.has(uuid)) {
			return;
		}
		setTimeout(() => {
			const element = document.getElementById(`chat-message-${uuid}`);
			if (element) {
				element.scrollIntoView({ behavior: 'smooth', block: 'center' });
				scrolledMessages.add(uuid);
			}
		}, delay);
	}

	$effect(() => {
		if (messageUuid && isFound(patchEvents.current)) {
			scrollToMessageWithDelay(messageUuid, 300);
		}
	});
</script>

{#if minimized}
	<ShowChatButton onclick={onMinimizeToggle} />
{:else}
	<div class="chat-wrapper" class:tablet-mode={isTabletMode}>
		<div class="chat-header">
			<h3 class="text-13 text-bold">Discussion</h3>
			<div class="chat-header-actions">
				<Button
					icon="minus-small"
					kind="ghost"
					tooltip="Hide discussion"
					onclick={onMinimizeToggle}
				/>
			</div>
		</div>

		<div class="chat-card">
			<div class="chat-messages">
				<Loading loadable={patchEvents.current}>
					{#snippet children(patchEvents)}
						{#if patchEvents.events.length > 0}
							{#each patchEvents.events as event}
								<Event {projectId} {changeId} {event} highlightedMessageUuid={messageUuid} />
							{/each}
						{:else}
							<div class="blank-state">
								<div class="blank-state-content">
									{@html blankChat}
									<div class="blank-message">
										<div class="text-18 text-semibold blank-message-title">Give some feedback!</div>
										<p class="text-12 text-body blank-message-text">
											If you're here, you must be important. This patch can use your help. Leave a
											comment or ask a question. Does this look right to you? How can it be
											improved? Is it perfect? Just let us know!
										</p>
									</div>
								</div>
							</div>
						{/if}
					{/snippet}
				</Loading>
			</div>
			<ChatInput
				{isUserLoggedIn}
				{branchUuid}
				{projectId}
				{branchId}
				{changeId}
				{isPatchAuthor}
				{diffSelection}
				{clearDiffSelection}
			/>
		</div>
	</div>
{/if}

<style lang="postcss">
	.chat-wrapper {
		width: 100%;
		display: flex;
		flex-direction: column;

		border-radius: var(--radius-ml, 10px);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);

		&.tablet-mode {
			border-radius: 0;
		}
	}

	.chat-card {
		width: 100%;
		height: 100%;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
	}

	.chat-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 10px 10px 10px 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.chat-header-actions {
		display: flex;
		gap: 2px;
	}

	.chat-messages {
		flex: 1;
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
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 50px 24px;
	}

	.blank-state-content {
		display: flex;
		flex-direction: column;
		gap: 28px;
		max-width: 420px;
	}

	.blank-message {
		padding-left: 17px;
	}

	.blank-message-title {
		margin-top: 10px;
	}

	.blank-message-text {
		color: var(--clr-text-2);
		margin-top: 10px;
	}
</style>
