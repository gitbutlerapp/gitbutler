<script lang="ts">
	import MessageHandler from '$lib/chat/message.svelte';
	import RichText from '$lib/chat/richText.svelte';
	import SuggestionsHandler from '$lib/chat/suggestions.svelte';
	import ChatDiffLineSelection from '$lib/components/chat/ChatDiffLineSelection.svelte';
	import ChatInReplyTo, { type ReplyToMessage } from '$lib/components/chat/ChatInReplyTo.svelte';
	import MentionSuggestions from '$lib/components/chat/MentionSuggestions.svelte';
	import { type DiffSelection } from '$lib/diff/lineSelection.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { CHAT_CHANNELS_SERVICE } from '@gitbutler/shared/chat/chatChannelsService';
	import { uploadFiles } from '@gitbutler/shared/dom';
	import { PATCH_COMMIT_SERVICE } from '@gitbutler/shared/patches/patchCommitService';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { UPLOADS_SERVICE } from '@gitbutler/shared/uploads/uploadsService';
	import { NEW_USER_SERVICE } from '@gitbutler/shared/users/userService';

	import {
		Button,
		ContextMenuItem,
		ContextMenuSection,
		DropdownButton,
		EmojiPickerButton,
		Mention as MentionsPlugin,
		RichTextEditor
	} from '@gitbutler/ui';
	import FileUploadPlugin, {
		type DropFileResult
	} from '@gitbutler/ui/richText/plugins/FileUpload.svelte';

	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';
	import { env } from '$env/dynamic/public';

	const ACCEPTED_FILE_TYPES = ['image/*', 'application/*', 'text/*', 'audio/*', 'video/*'];

	interface Props {
		projectId: string;
		branchId: string;
		branchUuid: string;
		changeId: string;
		patchCommit: PatchCommit;
		isPatchAuthor: boolean | undefined;
		isUserLoggedIn: boolean | undefined;
		diffSelection: DiffSelection | undefined;
		clearDiffSelection: () => void;
		replyingTo: ReplyToMessage | undefined;
		clearReply: () => void;
	}

	let {
		branchUuid,
		projectId,
		branchId,
		changeId,
		patchCommit,
		isPatchAuthor,
		isUserLoggedIn,
		diffSelection,
		clearDiffSelection,
		replyingTo,
		clearReply
	}: Props = $props();

	const newUserService = inject(NEW_USER_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	const appState = inject(APP_STATE);
	const patchCommitService = inject(PATCH_COMMIT_SERVICE);
	const chatChannelService = inject(CHAT_CHANNELS_SERVICE);
	const uploadsService = inject(UPLOADS_SERVICE);
	const contributors = $derived(patchCommit.contributors.map((c) => c.user).filter(isDefined));
	const chatParticipants = $derived(
		getChatChannelParticipants(appState, chatChannelService, projectId, changeId)
	);

	let isSendingMessage = $state(false);
	let isExecuting = $state(false);
	let fileUploadPlugin = $state<ReturnType<typeof FileUploadPlugin>>();

	const richText = new RichText();
	const messageHandler = new MessageHandler();
	const suggestions = new SuggestionsHandler();

	$effect(() => messageHandler.init(chatChannelService, projectId, branchId, changeId));
	$effect(() => suggestions.init(newUserService, chatParticipants.current, contributors, $user));
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
			await messageHandler.send({ issue, diffSelection, inReplyTo: replyingTo?.uuid });
		} finally {
			richText.clearEditor();
			isSendingMessage = false;
			clearDiffSelection();
			clearReply();
		}
	}

	function handleKeyDown(event: KeyboardEvent | null): boolean {
		if (event === null) return false;

		if (suggestions.suggestions !== undefined) {
			return suggestions.onSuggestionKeyDown(event);
		}

		const metaKey = event.metaKey || event.ctrlKey;

		if (event.key === 'Enter' && !event.shiftKey && suggestions.suggestions === undefined) {
			event.preventDefault();
			event.stopPropagation();
			handleSendMessage();
			return true;
		}

		if (event.key === 'Escape') {
			if (!diffSelection || metaKey) {
				// Clear reply only if the diff selection is not open
				// or if the meta key is pressed
				clearReply();
			}

			// Clear diff selection on escape only if the mention suggestions
			// are not open
			clearDiffSelection();
			return false;
		}

		if (event.key === 'Backspace' && !messageHandler.message) {
			if (!diffSelection || metaKey) {
				// Clear reply only if the diff selection is not open
				// or if the meta key is pressed
				clearReply();
			}

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
	let dropDownButton = $state<ReturnType<typeof DropdownButton>>();

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

	function isAcceptedFileType(file: File): boolean {
		const type = file.type.split('/')[0];
		return ACCEPTED_FILE_TYPES.some((acceptedType) => acceptedType.startsWith(type));
	}

	async function handleDropFiles(files: FileList | undefined): Promise<DropFileResult[]> {
		if (files === undefined) return [];
		const uploads = Array.from(files)
			.filter(isAcceptedFileType)
			.map(async (file) => {
				const upload = await uploadsService.uploadFile(file);

				return { name: file.name, url: upload.url, isImage: upload.isImage };
			});
		const settled = await Promise.allSettled(uploads);
		const successful = settled.filter((result) => result.status === 'fulfilled');
		return successful.map((result) => result.value);
	}

	async function attachFiles() {
		richText.richTextEditor?.focus();

		const files = await uploadFiles(ACCEPTED_FILE_TYPES.join(','));

		if (!files) return;
		await fileUploadPlugin?.handleFileUpload(files);
	}

	function onEmojiSelect(unicode: string) {
		richText.richTextEditor?.insertText(unicode);
	}

	export function focusInput() {
		richText.richTextEditor?.focus();
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
			{#if replyingTo}
				<div class="chat-input__reply">
					<ChatInReplyTo message={replyingTo} {clearReply} />
				</div>
			{/if}

			{#if diffSelection}
				<ChatDiffLineSelection {diffSelection} {clearDiffSelection} />
			{/if}

			<RichTextEditor
				styleContext="chat-input"
				placeholder="Write your message"
				bind:this={richText.richTextEditor}
				namespace="ChatInput"
				onError={console.error}
				onInput={(text) => messageHandler.update(text)}
				onKeyDown={handleKeyDown}
			>
				{#snippet plugins()}
					<MentionsPlugin
						bind:this={suggestions.mentionPlugin}
						getSuggestionItems={(q) => suggestions.getSuggestionItems(q)}
						onUpdateSuggestion={(p) => suggestions.onSuggestionUpdate(p)}
						onExitSuggestion={() => suggestions.onSuggestionExit()}
					/>
					<FileUploadPlugin bind:this={fileUploadPlugin} onDrop={handleDropFiles} />
				{/snippet}
			</RichTextEditor>
			<div class="chat-input__actions">
				<div class="chat-input__inner-toolbar">
					<EmojiPickerButton onEmojiSelect={(emoji) => onEmojiSelect(emoji.unicode)} />
					<div class="chat-input__inner-toolbar__divider"></div>
					<div class="chat-input__inner-toolbar__shrinkable">
						<Button
							kind="ghost"
							icon="attachment-small"
							reversedDirection
							onclick={attachFiles}
							shrinkable
							width="100%"
						>
							<span style="opacity: 0.4">Paste or drop to add files</span>
						</Button>
					</div>
				</div>

				<div class="chat-input__action-buttons">
					{#if isPatchAuthor === false}
						<DropdownButton
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
						</DropdownButton>
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
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}

	.chat-input__reply {
		padding: 6px 6px 0;
	}

	.chat-input__content-container {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
		border-radius: var(--radius-m);
	}

	.chat-input__actions {
		display: flex;
		position: relative;
		flex-grow: 1;
		justify-content: space-between;
		padding: 12px;
		gap: 12px;

		&:after {
			position: absolute;
			top: 0;
			left: 12px;
			width: calc(100% - 24px);
			height: 1px;
			background-color: var(--clr-border-3);
			content: '';
		}
	}

	.chat-input__inner-toolbar {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		overflow: hidden;
		gap: 6px;
	}

	.chat-input__inner-toolbar__divider {
		width: 1px;
		height: 18px;
		background-color: var(--clr-border-3);
	}

	.chat-input__action-buttons {
		display: flex;
		gap: 4px;
	}

	.chat-input__inner-toolbar__shrinkable {
		display: grid;
		overflow: hidden;
	}

	.chat-input-notlooged {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 16px;
		gap: 16px;
		border-top: 1px solid var(--clr-border-2);

		p {
			width: 100%;
			color: var(--clr-text-2);
			line-height: 140%;
		}
	}
</style>
