<script lang="ts">
	import MentionSuggestions from './MentionSuggestions.svelte';
	import { embedUserMention } from '$lib/chat/mentions';
	import RichText from '$lib/chat/richText.svelte';
	import SuggestionsHandler from '$lib/chat/suggestions.svelte';
	import { UserService } from '$lib/user/userService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import RichTextEditor, { type EditorInstance } from '@gitbutler/ui/RichTextEditor.svelte';

	interface Props {
		projectId: string;
		branchId: string;
		branchUuid: string;
		changeId: string;
	}

	let { branchUuid, projectId, branchId, changeId }: Props = $props();

	const newUserService = getContext(NewUserService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);
	const chatChannelService = getContext(ChatChannelsService);
	const chatParticipants = $derived(
		getChatChannelParticipants(appState, chatChannelService, projectId, changeId)
	);
	const suggestions = $derived(
		new SuggestionsHandler(newUserService, chatParticipants.current, $user)
	);

	let message = $state<string>();
	let isSendingMessage = $state(false);
	let isExecuting = $state(false);

	// Rich text editor
	const richText = new RichText();
	$effect(() => {
		if (changeId) {
			// Just here to track the changeId
		}

		return () => {
			// Cleanup once the change ID changes
			richText.reset();
		};
	});

	async function sendMessage(message: string | undefined, issue?: boolean) {
		if (message === undefined || message.trim() === '') {
			return;
		}

		await chatChannelService.sendChatMessage({
			projectId,
			branchId,
			changeId,
			chat: message,
			issue
		});
	}

	async function handleSendMessage(issue?: boolean) {
		if (isSendingMessage) return;
		isSendingMessage = true;
		try {
			await sendMessage(message, issue);
		} finally {
			const editor = richText.richTextEditor?.getEditor();
			editor?.commands.clearContent(true);
			isSendingMessage = false;
		}
	}

	function handleKeyDown(event: KeyboardEvent): boolean {
		if (event.key === 'Enter' && !event.shiftKey && richText.suggestions === undefined) {
			event.preventDefault();
			event.stopPropagation();
			handleSendMessage();
			return true;
		}

		const editor = richText.richTextEditor?.getEditor();
		if (event.key === 'Enter' && event.shiftKey && editor) {
			editor.commands.first(({ commands }) => [
				() => commands.newlineInCode(),
				() => commands.createParagraphNear(),
				() => commands.liftEmptyBlock(),
				() => commands.splitBlock()
			]);
			return true;
		}

		return false;
	}

	async function handleClickSend() {
		await handleSendMessage();
	}

	const actionLabels = {
		approve: 'Approve commit',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;

	let action = $state<Action>('approve');
	let dropDownButton = $state<ReturnType<typeof DropDownButton>>();

	async function approve() {
		await patchService.updatePatch(branchUuid, changeId, { signOff: true, message });
		const editor = richText.richTextEditor?.getEditor();
		editor?.commands.clearContent(true);
	}

	async function requestChanges() {
		await patchService.updatePatch(branchUuid, changeId, { signOff: false });
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
					await handleSendMessage(true);
					await requestChanges();
					break;
			}
		} finally {
			isExecuting = false;
		}
	}

	const actionButtonLabel = $derived.by(() => {
		const suffix = message ? ' & Comment' : '';
		return actionLabels[action] + suffix;
	});

	function onEditorUpdate(editor: EditorInstance) {
		message = editor?.getText({
			textSerializers: {
				mention: ({ node }) => {
					const id = node.attrs.id;
					const username = node.attrs.label;
					if (!id) {
						return '@' + username;
					}

					return embedUserMention(id);
				}
			}
		});
	}
</script>

<div class="chat-input">
	<MentionSuggestions
		bind:this={richText.mentionSuggestions}
		isLoading={suggestions.isLoading}
		suggestions={richText.suggestions}
		selectSuggestion={richText.selectSuggestion}
	/>
	<div class="text-input chat-input__content-container">
		<RichTextEditor
			bind:this={richText.richTextEditor}
			getSuggestionItems={(q) => suggestions.getSuggestionItems(q)}
			onSuggestionStart={(p) => richText.onSuggestionStart(p)}
			onSuggestionUpdate={(p) => richText.onSuggestionUpdate(p)}
			onSuggestionExit={() => richText.onSuggestionExit()}
			onSuggestionKeyDown={(event) => richText.onSuggestionKeyDown(event)}
			onKeyDown={handleKeyDown}
			onUpdate={onEditorUpdate}
		/>
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
						</ContextMenuSection>
					{/snippet}
				</DropDownButton>
				<Button
					style="pop"
					loading={isSendingMessage || isExecuting}
					disabled={!message}
					onclick={handleClickSend}>Comment</Button
				>
			</div>
		</div>
	</div>
</div>

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
</style>
