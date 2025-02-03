<script lang="ts">
	import MentionSuggestions from './MentionSuggestions.svelte';
	import RichText from '$lib/chat/richText.svelte';
	import { UserService } from '$lib/user/userService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getChatChannelParticipants } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import type { UserSimple } from '@gitbutler/shared/users/types';

	interface Props {
		projectId: string;
		branchId: string;
		branchUuid: string;
		changeId: string;
	}

	let { branchUuid, projectId, branchId, changeId }: Props = $props();

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);
	const chatChannelService = getContext(ChatChannelsService);
	const chatParticipants = $derived(
		getChatChannelParticipants(appState, chatChannelService, projectId, changeId)
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
			const editor = richText.richTextEditor?.getEditor();
			editor?.commands.clearContent();
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

	const userMap = $derived.by(() => {
		const map = new Map<string, UserSimple>();
		chatParticipants.current?.forEach((participant) => {
			if (!participant.login) return;
			map.set(participant.login, participant);
		});
		return map;
	});

	async function getSuggestionItems(query: string): Promise<string[]> {
		return (
			chatParticipants.current
				?.map((participant) => participant.login)
				.filter((username): username is string => !!username && username !== $user?.login)
				.filter((item) => item.toLowerCase().startsWith(query.toLowerCase())) ?? []
		);
	}
</script>

<div class="chat-input">
	<MentionSuggestions
		bind:this={richText.mentionSuggestions}
		suggestions={richText.suggestions}
		selectSuggestion={richText.selectSuggestion}
	>
		{#snippet item(username)}
			<div class="mention-suggestion">
				{#if userMap.has(username)}
					<img src={userMap.get(username)?.avatarUrl} alt={username} class="avatar" />
				{/if}
				<p class=" text-12 text-semibold name truncate">
					{username}
				</p>
			</div>
		{/snippet}
	</MentionSuggestions>
	<div class="chat-input__content-container">
		<RichTextEditor
			bind:this={richText.richTextEditor}
			{getSuggestionItems}
			onSuggestionStart={(p) => richText.onSuggestionStart(p)}
			onSuggestionUpdate={(p) => richText.onSuggestionUpdate(p)}
			onSuggestionExit={() => richText.onSuggestionExit()}
			onSuggestionKeyDown={(event) => richText.onSuggestionKeyDown(event)}
			onKeyDown={handleKeyDown}
			onTextUpdate={(text) => (message = text)}
		/>
		<div class="chat-input__actions">
			<div class="chat-input__secondary-actions">
				<p>📋</p>
				<p>😈</p>
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
		gap: 12px;
		padding: 12px;

		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
	}

	.chat-input__actions {
		flex-grow: 1;
		display: flex;
		justify-content: space-between;
	}

	.chat-input__secondary-actions {
		display: flex;

		p {
			padding: 6px;
			opacity: 0.5;
		}
	}

	.chat-input__action-buttons {
		display: flex;
		gap: 4px;
	}

	.mention-suggestion {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.avatar {
		width: 16px;
		height: 16px;
		border-radius: 50%;
	}
</style>
