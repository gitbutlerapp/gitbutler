<script lang="ts">
	import ChatDiffLineSelection from './ChatDiffLineSelection.svelte';
	import MentionSuggestions from './MentionSuggestions.svelte';
	import MessageHandler from '$lib/chat/message.svelte';
	import RichText from '$lib/chat/richText.svelte';
	import SuggestionsHandler from '$lib/chat/suggestions.svelte';
	import { type DiffSelection } from '$lib/diff/lineSelection.svelte';
	import { UserService } from '$lib/user/userService';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { PatchCommitService } from '@gitbutler/shared/patches/patchCommitService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import MentionsPlugin from '@gitbutler/ui/richText/plugins/Mention.svelte';
	import { env } from '$env/dynamic/public';

	interface Props {
		projectId: string;
		branchId: string;
		branchUuid: string;
		changeId: string;
		isPatchAuthor: boolean | undefined;
		isUserLoggedIn: boolean | undefined;
		diffSelection: DiffSelection | undefined;
		clearDiffSelection: () => void;
	}

	let {
		branchUuid,
		projectId,
		branchId,
		changeId,
		isPatchAuthor,
		isUserLoggedIn,
		diffSelection,
		clearDiffSelection
	}: Props = $props();

	const newUserService = getContext(NewUserService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const appState = getContext(AppState);
	const patchCommitService = getContext(PatchCommitService);
	const chatChannelService = getContext(ChatChannelsService);
	const chatParticipants = $derived(
		getChatChannelParticipants(appState, chatChannelService, projectId, changeId)
	);

	let isSendingMessage = $state(false);
	let isExecuting = $state(false);

	const richText = new RichText();
	const messageHandler = new MessageHandler();
	const suggestions = new SuggestionsHandler();

	$effect(() => messageHandler.init(chatChannelService, projectId, branchId, changeId));
	$effect(() => suggestions.init(newUserService, chatParticipants.current, $user));
	$effect(() => {
		if (changeId) {
			// Just here to track the changeId
		}

		return () => {
			// Cleanup once the change ID changes
			richText.clearEditor();
			suggestions.reset();
		};
	});

	async function handleSendMessage(issue?: boolean) {
		if (isSendingMessage) return;
		isSendingMessage = true;
		try {
			await messageHandler.send({ issue, diffSelection });
		} finally {
			richText.clearEditor();
			isSendingMessage = false;
			clearDiffSelection();
		}
	}

	function handleKeyDown(event: KeyboardEvent | null): boolean {
		if (event === null) return false;

		if (suggestions.suggestions !== undefined) {
			return suggestions.onSuggestionKeyDown(event);
		}

		if (event.key === 'Enter' && !event.shiftKey && suggestions.suggestions === undefined) {
			event.preventDefault();
			event.stopPropagation();
			handleSendMessage();
			return true;
		}

		if (event.key === 'Escape') {
			// Clear diff selection on escape only if the mention suggestions
			// are not open
			clearDiffSelection();
			return false;
		}

		if (event.key === 'Backspace' && !messageHandler.message) {
			// Clear diff selection on delete only if the mention suggestions
			// are not open and the input is empty
			clearDiffSelection();
			return false;
		}

		return false;
	}

	async function handleClickSend() {
		await handleSendMessage();
	}

	const actionLabels = {
		approve: 'Approve commit',
		openIssue: 'Open issue',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;

	let action = $state<Action>('approve');
	let dropDownButton = $state<ReturnType<typeof DropDownButton>>();

	async function approve() {
		await patchCommitService.updatePatch(branchUuid, changeId, {
			signOff: true,
			message: messageHandler.message
		});
		richText.clearEditor();
	}

	async function requestChanges() {
		await patchCommitService.updatePatch(branchUuid, changeId, {
			signOff: false,
			message: messageHandler.message
		});
		richText.clearEditor();
	}

	async function handleActionClick() {
		if (isExecuting) return;
		isExecuting = true;
		try {
			switch (action) {
				case 'approve':
					await approve();
					break;
				case 'requestChanges':
					await requestChanges();
					break;
				case 'openIssue':
					await handleSendMessage(true);
					break;
			}
		} finally {
			isExecuting = false;
		}
	}

	const actionButtonLabel = $derived.by(() => {
		const suffix = messageHandler.message ? ' & Comment' : '';
		return actionLabels[action] + suffix;
	});

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/login?callback=${window.location.href}`;
	}
</script>

{#if isUserLoggedIn}
	<div class="chat-input">
		<MentionSuggestions
			bind:this={suggestions.mentionSuggestions}
			isLoading={suggestions.isLoading}
			suggestions={suggestions.suggestions}
			selectSuggestion={(s) => suggestions.selectSuggestion(s)}
		/>
		<div class="text-input chat-input__content-container">
			{#if diffSelection}
				<ChatDiffLineSelection {diffSelection} {clearDiffSelection} />
			{/if}
			<RichTextEditor
				bind:this={richText.richTextEditor}
				markdown={false}
				namespace="ChatInput"
				onError={console.error}
				onChange={(text) => messageHandler.update(text)}
				onKeyDown={handleKeyDown}
			>
				{#snippet plugins()}
					<MentionsPlugin
						bind:this={suggestions.mentionPlugin}
						getSuggestionItems={(q) => suggestions.getSuggestionItems(q)}
						onUpdateSuggestion={(p) => suggestions.onSuggestionUpdate(p)}
						onExitSuggestion={() => suggestions.onSuggestionExit()}
					/>
				{/snippet}
			</RichTextEditor>
			<div class="chat-input__actions">
				<div class="chat-input__secondary-actions">
					<Button
						icon="attachment"
						tooltip="Attach files"
						tooltipPosition="top"
						kind="ghost"
						disabled
						onclick={() => {
							// TODO: Implement
						}}
					/>
					<Button
						icon="smile"
						kind="ghost"
						tooltipPosition="top"
						tooltip="Insert emoji"
						disabled
						onclick={() => {
							// TODO: Implement
						}}
					/>
				</div>
				<div class="chat-input__action-buttons">
					{#if isPatchAuthor === false}
						<DropDownButton
							bind:this={dropDownButton}
							loading={isSendingMessage || isExecuting}
							style="neutral"
							kind="outline"
							onclick={handleActionClick}
						>
							{actionButtonLabel}
							{#snippet contextMenuSlot()}
								<ContextMenuSection>
									<ContextMenuItem
										label={actionLabels.approve}
										onclick={() => {
											action = 'approve';
											dropDownButton?.close();
										}}
									/>
									<ContextMenuItem
										label={actionLabels.requestChanges}
										onclick={() => {
											action = 'requestChanges';
											dropDownButton?.close();
										}}
									/>
									<ContextMenuItem
										label={actionLabels.openIssue}
										onclick={() => {
											action = 'openIssue';
											dropDownButton?.close();
										}}
									/>
								</ContextMenuSection>
							{/snippet}
						</DropDownButton>
					{/if}
					<Button
						style="pop"
						loading={isSendingMessage || isExecuting}
						disabled={!messageHandler.message}
						onclick={handleClickSend}>Comment</Button
					>
				</div>
			</div>
		</div>
	</div>
{:else}
	<div class="chat-input-notlooged">
		<p class="text-12">🔒 You must be logged in to join the conversation</p>
		<Button style="pop" onclick={login}>Log in to comment</Button>
	</div>
{/if}

<style lang="postcss">
	.chat-input {
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}

	.chat-input__content-container {
		flex-grow: 1;
		display: flex;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
	}

	.chat-input__actions {
		flex-grow: 1;
		display: flex;
		padding: 12px;
		padding-top: 0;
		justify-content: space-between;
	}

	.chat-input__secondary-actions {
		display: flex;
	}

	.chat-input__action-buttons {
		display: flex;
		gap: 4px;
	}

	.chat-input-notlooged {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: 16px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);

		p {
			width: 100%;
			color: var(--clr-text-2);
			line-height: 140%;
		}
	}
</style>
