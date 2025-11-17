<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import AttachmentList from '$components/codegen/AttachmentList.svelte';
	import CodegenQueued from '$components/codegen/CodegenQueued.svelte';
	import FileSearch from '$components/codegen/FileSearch.svelte';
	import { BACKEND } from '$lib/backend';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import {
		CodegenCommitDropHandler,
		CodegenFileDropHandler,
		CodegenHunkDropHandler
	} from '$lib/codegen/dropzone';
	import { type UserInput, type PromptAttachment, type ClaudeMessage } from '$lib/codegen/types';
	import { newlineOnEnter } from '$lib/config/uiFeatureFlags';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { showError } from '$lib/notifications/toasts';
	import { inject } from '@gitbutler/core/context';
	import { Tooltip, AsyncButton, RichTextEditor, FilePlugin, UpDownPlugin } from '@gitbutler/ui';
	import { tick, type Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { FileSuggestionUpdate } from '@gitbutler/ui/richText/plugins/FilePlugin.svelte';

	type Props = {
		projectId: string;
		branchName: string;
		stackId?: string;
		value: string;
		loading: boolean;
		compacting: boolean;
		onSubmit?: (text: string) => Promise<void>;
		onAbort?: () => Promise<void>;
		onCancel?: () => void;
		actionsOnLeft: Snippet;
		actionsOnRight: Snippet;
		onChange: (value: string) => void;
	};

	let {
		projectId,
		stackId,
		branchName,
		value = $bindable(),
		loading,
		compacting,
		onSubmit,
		onAbort,
		onCancel,
		actionsOnLeft,
		actionsOnRight,
		onChange
	}: Props = $props();

	const backend = inject(BACKEND);
	const attachmentService = inject(ATTACHMENT_SERVICE);
	const attachments = $derived(attachmentService.getByBranch(branchName));

	let editorRef = $state<ReturnType<typeof RichTextEditor>>();
	let showAbortButton = $state(false);

	const fileService = inject(FILE_SERVICE);
	let indexOfSelectedFile = $state<number>();
	let fileSuggestionsPlugin = $state<FilePlugin>();
	let loadingFileSuggestions = $state(false);
	let fileSuggestions = $state<string[] | undefined>(undefined);
	let fileSuggestionsQuery = $state<string>('');

	function selectFileSuggestion(filename: string) {
		fileSuggestionsPlugin?.selectFileSuggestion(filename);
	}

	function exitFileSuggestions() {
		fileSuggestionsPlugin?.exitFileSuggestions();
	}

	function onFileSuggestionUpdate(update: FileSuggestionUpdate, query: string) {
		fileSuggestionsQuery = query;
		if (update.loading) {
			loadingFileSuggestions = true;
			fileSuggestions = [];
			indexOfSelectedFile = undefined;
			return;
		}
		loadingFileSuggestions = false;
		fileSuggestions = update.items;
		if (update.items.length === 0) {
			indexOfSelectedFile = undefined;
		} else {
			indexOfSelectedFile = 0;
		}
	}

	function handleFileSuggestionsKeyDown(event: KeyboardEvent, fileSuggestions: string[]): boolean {
		const { key } = event;
		const i = indexOfSelectedFile ?? 0;

		switch (key) {
			case 'ArrowUp':
				if (fileSuggestions.length === 0) return false;
				event.stopPropagation();
				event.preventDefault();
				indexOfSelectedFile = i === 0 ? fileSuggestions.length - 1 : i - 1;
				return true;
			case 'ArrowDown':
				if (fileSuggestions.length === 0) return false;
				event.stopPropagation();
				event.preventDefault();
				indexOfSelectedFile = (i + 1) % fileSuggestions.length;
				return true;
			case 'Enter': {
				if (fileSuggestions.length === 0) return false;
				event.stopPropagation();
				event.preventDefault(); // Prevents newline in editor.
				const file = fileSuggestions[indexOfSelectedFile ?? 0];
				if (file) {
					selectFileSuggestion(file);
				}
				return true;
			}
			case 'Escape':
				exitFileSuggestions();
				return true;
			default:
				return false;
		}
	}

	$effect(() => {
		// Show abort button if loading for more than 1 second
		if (loading && onAbort) {
			const timer = setTimeout(() => {
				showAbortButton = true;
			}, 1000);

			return () => {
				clearTimeout(timer);
				showAbortButton = false;
			};
		} else {
			showAbortButton = false;
		}
	});

	async function handleSubmit() {
		const text = await editorRef?.getPlaintext();
		if (!text || text.trim().length === 0) return;
		const state = editorRef?.save();
		try {
			// We can't rely on set prompt updating prompt text in `laneState`
			// for clearing the input, so we do it here to keep them in sync.
			// TODO: Make it so that updating laneState resets the editor.
			editorRef?.clear();
			await onSubmit?.(text);
		} catch (err) {
			if (state) {
				editorRef?.load(state);
			}
			showError('Send error', err);
		}
	}

	function handleEditorKeyDown(event: KeyboardEvent | null): boolean {
		if (!event) return false;
		if (fileSuggestions) {
			return handleFileSuggestionsKeyDown(event, fileSuggestions);
		}

		// Handle Escape to cancel/close
		if (event.key === 'Escape' && onCancel) {
			event.preventDefault();
			onCancel();
			return true;
		}

		// Handle Ctrl+C to abort
		if (event.key === 'c' && event.ctrlKey && onAbort) {
			event.preventDefault();
			onAbort();
			return true;
		}

		// Handle Enter to submit
		if (event.key === 'Enter' && !event.shiftKey && (event.metaKey || !$newlineOnEnter)) {
			event.preventDefault();
			handleSubmit();
			return true;
		}
		return false;
	}

	function addAttachment(items: PromptAttachment[]) {
		return attachmentService.add(branchName, items);
	}

	const handlers = $derived([
		new CodegenCommitDropHandler(stackId, addAttachment),
		new CodegenFileDropHandler(stackId, branchName, addAttachment),
		new CodegenHunkDropHandler(stackId, addAttachment)
	]);

	// Expose methods to manipulate text programmatically (e.g., for template insertion)
	export function setText(text: string) {
		editorRef?.setText(text);
	}

	export function getText() {
		return editorRef?.getPlaintext();
	}
</script>

<div class="dialog-wrapper">
	<FileSearch
		{indexOfSelectedFile}
		loading={loadingFileSuggestions}
		onselect={selectFileSuggestion}
		onexit={exitFileSuggestions}
		files={fileSuggestions}
		query={fileSuggestionsQuery}
	/>

	<div
		class="text-input dialog-input"
		data-remove-from-panning
		role="button"
		tabindex="-1"
		onclick={() => editorRef?.focus()}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') {
				editorRef?.focus();
			}
		}}
	>
		<CodegenQueued {projectId} laneId={stackId} {branchName} />

		<Dropzone {handlers}>
			{#snippet overlay({ hovered, activated })}
				<CardOverlay {hovered} {activated} label="Reference" />
			{/snippet}
			{#if attachments.length > 0}
				<div class="attached-files-section">
					<AttachmentList
						{attachments}
						onRemove={(a) => attachmentService.removeByBranch(branchName, a)}
					/>
				</div>
			{/if}

			<RichTextEditor
				bind:this={editorRef}
				bind:value
				namespace="codegen-input"
				plaintext={true}
				styleContext="chat-input"
				placeholder="Use @ to reference files, ↓ and ↑ for prompt history"
				minHeight="4rem"
				maxHeight="20rem"
				onError={(e: unknown) => console.warn('Editor error', e)}
				initialText={value}
				{onChange}
				onKeyDown={handleEditorKeyDown}
			>
				{#snippet plugins()}
					<FilePlugin
						bind:this={fileSuggestionsPlugin}
						getFileItems={(q: string) => fileService.fetchFiles(projectId, q, 8)}
						onUpdateSuggestion={onFileSuggestionUpdate}
						onExitSuggestion={() => {
							fileSuggestions = undefined;
						}}
					/>
					<UpDownPlugin
						historyLookup={async (offset) => {
							attachmentService.clearByBranch(branchName);
							const resp = await backend.invoke<
								ClaudeMessage<{ source: 'user' } & UserInput> | undefined
							>('claude_get_user_message', {
								projectId,
								offset
							});
							// Let the state update so payload is removed.
							await tick();
							const payload = resp?.payload;
							if (payload) {
								const attachments = payload.attachments;
								if (attachments) {
									attachmentService.add(branchName, attachments);
								}
								return payload.message;
							}
						}}
					/>
				{/snippet}
			</RichTextEditor>

			<div role="presentation" class="actions" onclick={(e) => e.stopPropagation()}>
				<div class="actions-group">
					{@render actionsOnLeft()}
				</div>

				<div class="actions-group">
					{@render actionsOnRight()}

					<div class="actions-separator"></div>

					{#if !compacting && showAbortButton && onAbort}
						<div class="flex" in:fade={{ duration: 150 }} out:fade={{ duration: 100 }}>
							<AsyncButton
								kind="outline"
								style="error"
								action={onAbort}
								icon="stop"
								hotkey="⌃C"
								reversedDirection
							>
								Stop
							</AsyncButton>
						</div>
					{/if}

					<Tooltip
						text={loading
							? 'Processing...'
							: value.trim().length === 0
								? 'Type a message'
								: 'Send ⌘↵'}
					>
						<button
							class="send-button"
							type="button"
							class:loading
							style="pop"
							onclick={handleSubmit}
							aria-label="Send"
						>
							<svg
								class="circle-icon"
								viewBox="0 0 18 18"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<circle
									vector-effect="non-scaling-stroke"
									cx="9"
									cy="9"
									r="8.25"
									stroke="currentColor"
								/>
							</svg>

							<svg
								class="arrow-icon"
								viewBox="0 0 16 16"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									vector-effect="non-scaling-stroke"
									d="M12.0195 8L8.72664 4.70711C8.33611 4.31658 7.70295 4.31658 7.31242 4.70711L4.01953 8"
									stroke="currentColor"
									stroke-width="1.5"
								/>
								<path
									d="M8.01953 4L8.01953 12"
									stroke="currentColor"
									stroke-width="1.5"
									vector-effect="non-scaling-stroke"
								/>
							</svg>
						</button>
					</Tooltip>
				</div>
			</div>
		</Dropzone>
	</div>
</div>

<style lang="postcss">
	.dialog-wrapper {
		position: relative;
	}

	.dialog-input {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
		cursor: text;
		transition: border-color var(--transition-fast);
	}

	.attached-files-section {
		padding: 12px;
		padding-bottom: 0px;
	}

	.actions {
		display: flex;
		z-index: 2;
		position: relative;
		bottom: 0;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 12px;
		padding-top: 10px;
		gap: 8px;
		pointer-events: none;

		&::after {
			position: absolute;
			top: 0;
			right: 12px;
			left: 12px;
			height: 1px;
			background-color: var(--clr-border-3);
			content: '';
		}
	}

	.actions-group {
		display: flex;
		position: relative;
		gap: 4px;
		pointer-events: all;
	}

	.actions-separator {
		width: 1px;
		margin: 0 5px;
		background-color: var(--clr-border-3);
	}

	/* SEND BUTTON */
	.send-button {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		height: var(--size-button);
		padding: 0 6px;
		border-radius: var(--radius-btn);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		transition:
			background-color 0.2s ease-in-out,
			transform 0.2s ease-in-out;

		&:not(:disabled):hover {
			transform: translateY(-2px);
			background-color: var(--clr-theme-pop-element-hover);

			.arrow-icon {
				transform: translate(-50%, -50%) translateY(1px);
			}
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}
	}

	.circle-icon {
		width: 18px;
		height: 18px;
		stroke-width: 1.5;
		transition: transform 0.2s;
	}

	.arrow-icon {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 16px;
		height: 16px;
		transform: translate(-50%, -50%);
		transition: transform 0.2s;
	}
</style>
