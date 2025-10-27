<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import AttachmentList from '$components/codegen/AttachmentList.svelte';
	import CodegenQueued from '$components/codegen/CodegenQueued.svelte';
	import FileSearch from '$components/codegen/FileSearch.svelte';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import {
		CodegenCommitDropHandler,
		CodegenFileDropHandler,
		CodegenHunkDropHandler
	} from '$lib/codegen/dropzone';
	import { newlineOnEnter } from '$lib/config/uiFeatureFlags';
	import { showError } from '$lib/notifications/toasts';
	import { inject } from '@gitbutler/core/context';
	import { Tooltip, AsyncButton, RichTextEditor, FilePlugin } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';
	import type { PromptAttachment } from '$lib/codegen/types';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		branchName: string;
		stackId: string;
		value: string;
		loading: boolean;
		compacting: boolean;
		onsubmit: (text: string) => Promise<void>;
		onAbort?: () => Promise<void>;
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
		onsubmit,
		onAbort,
		actionsOnLeft,
		actionsOnRight,
		onChange
	}: Props = $props();

	const attachmentService = inject(ATTACHMENT_SERVICE);
	const attachments = $derived(attachmentService.getByBranch(branchName));

	let editorRef = $state<ReturnType<typeof RichTextEditor>>();
	let showAbortButton = $state(false);

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
			await onsubmit(text);
		} catch (err) {
			if (state) {
				editorRef?.load(state);
			}
			showError('Send error', err);
		}
	}

	function handleEditorKeyDown(event: KeyboardEvent | null): boolean {
		if (!event) return false;

		// Handle Ctrl+C to abort
		if (event.key === 'c' && event.ctrlKey && onAbort) {
			event.preventDefault();
			onAbort();
			return true;
		}

		// Handle Enter to submit
		if (event.key === 'Enter' && (event.metaKey || !$newlineOnEnter)) {
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
		new CodegenCommitDropHandler(stackId, branchName, addAttachment),
		new CodegenFileDropHandler(stackId, branchName, addAttachment),
		new CodegenHunkDropHandler(stackId, branchName, addAttachment)
	]);

	let query = $state('');
	let callback = $state<((result: string) => void) | undefined>();

	const placeholderVariants = [
		'What to build?',
		'Describe your changes.',
		'What to create?',
		'Describe what to build.',
		'What to code?'
	];
</script>

<div class="dialog-wrapper">
	<FileSearch {projectId} {query} onselect={callback} limit={8} />

	<div class="text-input dialog-input" data-remove-from-panning>
		<CodegenQueued {projectId} {stackId} {branchName} />

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

			{@const randomPlaceholder =
				placeholderVariants[Math.floor(Math.random() * placeholderVariants.length)]}
			<RichTextEditor
				bind:this={editorRef}
				bind:value
				namespace="codegen-input"
				markdown={false}
				styleContext="chat-input"
				placeholder="{randomPlaceholder} Use @ to reference files…"
				minHeight="4rem"
				onError={(e: unknown) => console.warn('Editor error', e)}
				initialText={value}
				{onChange}
				onKeyDown={handleEditorKeyDown}
			>
				{#snippet plugins()}
					<FilePlugin
						onQuery={(q: string, cb?: (result: string) => void) => {
							callback = cb;
							query = q;
						}}
					/>
				{/snippet}
			</RichTextEditor>

			<div class="actions">
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
		flex-shrink: 0;
		width: 100%;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}

	.dialog-input {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
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
