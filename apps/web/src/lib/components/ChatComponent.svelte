<script lang="ts">
	import blankChat from '$lib/assets/blank-chat.svg?raw';
	import ReplyHandler from '$lib/chat/reply.svelte';
	import ShowChatButton from '$lib/components/ShowChatButton.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import Event from '$lib/components/chat/Event.svelte';
	import { type DiffSelection } from '$lib/diff/lineSelection.svelte';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { PATCH_EVENTS_SERVICE } from '@gitbutler/shared/patchEvents/patchEventsService';
	import { getPatchEvents } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { Button } from '@gitbutler/ui';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';

	type Props = {
		messageUuid: string | undefined;
		isPatchAuthor: boolean | undefined;
		branchUuid: string;
		projectId: string;
		projectSlug: string;
		branchId: string;
		changeId: string;
		patchCommit: PatchCommit;
		minimized: boolean;
		isUserLoggedIn: boolean | undefined;
		isTabletMode: boolean;
		onMinimizeToggle: () => void;
		diffSelection: DiffSelection | undefined;
		clearDiffSelection: () => void;
	};

	let {
		messageUuid,
		projectId,
		projectSlug,
		changeId,
		branchId,
		patchCommit,
		branchUuid,
		minimized,
		isPatchAuthor,
		isUserLoggedIn,
		isTabletMode,
		onMinimizeToggle,
		diffSelection,
		clearDiffSelection
	}: Props = $props();

	const appState = inject(APP_STATE);
	const patchEventsService = inject(PATCH_EVENTS_SERVICE);
	const replyToHandler = new ReplyHandler();

	let highlightedMessageUuid = $state<string>();
	let chatInput = $state<ReturnType<typeof ChatInput>>();

	$effect(() => {
		if (changeId) {
			// Just here to track the changeId
		}
		return () => {
			// Cleanup
			replyToHandler.clear();
		};
	});

	const patchEvents = $derived(getPatchEvents(appState, patchEventsService, projectId, changeId));
	// This shouldn't be reactive as is just to check if the message was scrolled to already.
	// Only a hard reload should trigger the scroll again.
	const scrolledMessages = new Set<string>();

	function scrollToMessageWithDelay(uuid: string, delay: number, force?: boolean) {
		if (scrolledMessages.has(uuid) && !force) {
			return;
		}
		setTimeout(() => {
			const element = document.getElementById(`chat-message-${uuid}`);
			if (element) {
				element.scrollIntoView({ behavior: 'smooth', block: 'center' });
				scrolledMessages.add(uuid);
				highlightedMessageUuid = uuid;
			}
		}, delay);
	}

	$effect(() => {
		if (messageUuid && isFound(patchEvents.current)) {
			scrollToMessageWithDelay(messageUuid, 300);
		}
	});

	export function focus() {
		chatInput?.focusInput();
	}
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
							{#each patchEvents.events as event (event.uuid)}
								<Event
									{projectSlug}
									{projectId}
									{changeId}
									{event}
									{highlightedMessageUuid}
									replyTo={(event) => replyToHandler.replyTo(event.object)}
									scrollToMessage={(uuid) => scrollToMessageWithDelay(uuid, 0, true)}
								/>
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
				bind:this={chatInput}
				{isUserLoggedIn}
				{branchUuid}
				{projectId}
				{branchId}
				{changeId}
				{patchCommit}
				{isPatchAuthor}
				{diffSelection}
				{clearDiffSelection}
				replyingTo={replyToHandler.inReplyTo}
				clearReply={() => replyToHandler.clear()}
			/>
		</div>
	</div>
{/if}

<style lang="postcss">
	.chat-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-ml, 10px);
		background: var(--clr-bg-1);
		pointer-events: all;

		&.tablet-mode {
			border-radius: 0;
		}
	}

	.chat-card {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.chat-header {
		display: flex;
		position: sticky;
		top: 0;
		align-items: center;
		justify-content: space-between;
		padding: 10px 10px 10px 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.chat-header-actions {
		display: flex;
		gap: 2px;
	}

	.chat-messages {
		display: flex;
		flex: 1;
		flex-direction: column-reverse;
		overflow-y: scroll;
		scrollbar-width: none;

		&::-webkit-scrollbar {
			display: none;
		}
	}

	.blank-state {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		padding: 50px 24px;
	}

	.blank-state-content {
		display: flex;
		flex-direction: column;
		max-width: 420px;
		gap: 28px;
	}

	.blank-message {
		padding-left: 17px;
	}

	.blank-message-title {
		margin-top: 10px;
	}

	.blank-message-text {
		margin-top: 10px;
		color: var(--clr-text-2);
	}
</style>
