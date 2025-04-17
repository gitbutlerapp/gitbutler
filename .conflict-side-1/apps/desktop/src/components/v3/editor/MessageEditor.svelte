<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import MessageEditorRuler from '$components/v3/editor/MessageEditorRuler.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { uploadFiles } from '@gitbutler/shared/dom';
	import { persisted } from '@gitbutler/shared/persisted';
	import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
	import { debouncePromise } from '@gitbutler/shared/utils/misc';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import EmojiPickerButton from '@gitbutler/ui/EmojiPickerButton.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import FileUploadPlugin, {
		type DropFileResult
	} from '@gitbutler/ui/richText/plugins/FileUpload.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';
	import FormattingButton from '@gitbutler/ui/richText/tools/FormattingButton.svelte';

	const ACCEPTED_FILE_TYPES = ['image/*', 'application/*', 'text/*', 'audio/*', 'video/*'];

	interface Props {
		projectId: string;
		disabled?: boolean;
		initialValue?: string;
		placeholder: string;
		onChange?: (text: string) => void;
		onKeyDown?: (e: KeyboardEvent) => boolean;
		enableFileUpload?: boolean;
		onAiButtonClick: (e: MouseEvent) => void;
		canUseAI: boolean;
		aiIsLoading: boolean;
		suggestionsHandler?: CommitSuggestions;
	}

	let {
		initialValue,
		placeholder,
		disabled,
		enableFileUpload,
		onChange,
		onKeyDown,
		onAiButtonClick,
		canUseAI,
		aiIsLoading,
		suggestionsHandler
	}: Props = $props();

	const MIN_RULER_VALUE = 30;
	const MAX_RULER_VALUE = 200;

	const uiState = getContext(UiState);
	const uploadsService = getContext(UploadsService);

	const useRichText = uiState.global.useRichText;
	const useRuler = uiState.global.useRuler;
	const rulerCountValue = uiState.global.rulerCountValue;
	const wrapTextByRuler = uiState.global.wrapTextByRuler;

	const wrapCountValue = $derived(
		useRuler.current && wrapTextByRuler.current && !useRichText.current
			? rulerCountValue.current
			: undefined
	);

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();
	let isEditorHovered = $state(false);
	let isEditorFocused = $state(false);
	let fileUploadPlugin = $state<ReturnType<typeof FileUploadPlugin>>();
	let uploadConfirmationModal = $state<ReturnType<typeof Modal>>();
	const doNotShowUploadWarning = persisted<boolean>(false, 'doNotShowUploadWarning');
	let allowUploadOnce = $state<boolean>(false);

	export async function getPlaintext(): Promise<string | undefined> {
		return composer?.getPlaintext();
	}

	async function handleChange(
		text: string,
		textUpToAnchor: string | undefined,
		textAfterAnchor: string | undefined
	) {
		onChange?.(text);
		await suggestionsHandler?.onChange(textUpToAnchor, textAfterAnchor);
	}

	const debouncedHandleChange = debouncePromise(handleChange, 700);

	function handleKeyDown(event: KeyboardEvent | null) {
		if (event && onKeyDown?.(event)) {
			return true;
		}
		return suggestionsHandler?.onKeyDown(event) ?? false;
	}

	function onEmojiSelect(emoji: string) {
		composer?.insertText(emoji);
	}

	function isAcceptedFileType(file: File): boolean {
		const type = file.type.split('/')[0];
		if (!type) return false;
		return ACCEPTED_FILE_TYPES.some((acceptedType) => acceptedType.startsWith(type));
	}

	async function onDropFiles(files: FileList | undefined): Promise<DropFileResult[]> {
		if (files === undefined) return [];
		const uploads = Array.from(files)
			.filter(isAcceptedFileType)
			.map(async (file) => {
				const upload = await uploadsService.uploadFile(file);

				return { name: file.name, url: upload.url, isImage: upload.isImage };
			});
		const settled = await Promise.allSettled(uploads);
		const successful = settled.filter((result) => result.status === 'fulfilled');
		const failed = settled.filter((result) => result.status === 'rejected');

		if (failed.length > 0) {
			console.error('File upload failed', failed);
			showError('File upload failed', failed.map((result) => result.reason).join(', '));
		}

		allowUploadOnce = false;

		return successful.map((result) => result.value);
	}

	async function handleDropFiles(
		files: FileList | undefined
	): Promise<DropFileResult[] | undefined> {
		if ($doNotShowUploadWarning || allowUploadOnce) {
			return onDropFiles(files);
		}

		uploadConfirmationModal?.show('needs-re-drop');
		return undefined;
	}

	async function attachFiles() {
		composer?.focus();

		const files = await uploadFiles(ACCEPTED_FILE_TYPES.join(','));

		if (!files) return;
		await fileUploadPlugin?.handleFileUpload(files);
	}

	function handleAttachFiles() {
		if ($doNotShowUploadWarning) {
			attachFiles();
			return;
		}
		uploadConfirmationModal?.show();
	}

	export function focus() {
		composer?.focus();
	}

	export function setText(text: string) {
		composer?.setText(text);
	}
</script>

<Modal
	title="Dear sir and/or madam"
	width="small"
	bind:this={uploadConfirmationModal}
	onSubmit={async (close) => {
		allowUploadOnce = true;
		await attachFiles();
		close();
	}}
>
	{#snippet children(maybeCode)}
		<p>
			Thanks for your interest on attaching a file to this message.
			<br />
			<br />
			Before doing so, we'd like to make it clear that
			<b>the file would be uploaded to the GitButler servers.</b>
			<br />
			This is necessary to generate a link to the file that you can use in your message.
			<br />
			<br />
			Please note that we take your privacy seriously and will not share your file with any third parties.
			<br />
			The generated URL is yours to share and use as you see fit.
			<br />
			<br />
			<em>Best regards</em>,
			<br />
			<em>{'Your GitButler team <3'}</em>
		</p>
		<br />
		{#if maybeCode === 'needs-re-drop'}
			<p>
				<b>PS: Please re-drop the file to upload it.</b>
			</p>
			<br />
		{/if}
		<div style="display: flex; align-items: center; gap: 4px">
			<Checkbox small bind:checked={$doNotShowUploadWarning} />
			<span> Do not bring this up again </span>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>How dare you</Button>
		<Button style="pop" type="submit">Carry on</Button>
	{/snippet}
</Modal>

<div
	class="editor-wrapper"
	style:--lexical-input-client-text-wrap={useRuler.current && !useRichText.current
		? 'nowrap'
		: 'normal'}
>
	<div class="editor-header">
		<div class="editor-tabs">
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={!useRichText.current}
				class:focused={!useRichText.current && (isEditorFocused || isEditorHovered)}
				onclick={() => {
					useRichText.current = false;
				}}>Plain</button
			>
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={useRichText.current}
				class:focused={useRichText.current && (isEditorFocused || isEditorHovered)}
				onclick={() => {
					useRichText.current = true;
				}}>Rich-text</button
			>
		</div>

		<FormattingBar bind:formatter />
	</div>

	<div
		role="presentation"
		class="message-textarea"
		onmouseenter={() => (isEditorHovered = true)}
		onmouseleave={() => (isEditorHovered = false)}
	>
		<div
			role="presentation"
			class="message-textarea__inner"
			onclick={() => {
				composer?.focus();
			}}
		>
			{#if useRuler.current && !useRichText.current}
				<MessageEditorRuler />
			{/if}

			<ConfigurableScrollableContainer height="100%">
				<div class="message-textarea__wrapper">
					<RichTextEditor
						styleContext="client-editor"
						namespace="CommitMessageEditor"
						{placeholder}
						bind:this={composer}
						markdown={useRichText.current}
						onError={(e) => showError('Editor error', e)}
						initialText={initialValue}
						onChange={debouncedHandleChange}
						onKeyDown={handleKeyDown}
						onFocus={() => (isEditorFocused = true)}
						onBlur={() => (isEditorFocused = false)}
						{disabled}
						{wrapCountValue}
					>
						{#snippet plugins()}
							<Formatter bind:this={formatter} />
							<FileUploadPlugin bind:this={fileUploadPlugin} onDrop={handleDropFiles} />
							{#if suggestionsHandler}
								<GhostTextPlugin
									bind:this={suggestionsHandler.ghostTextComponent}
									onSelection={(text) => suggestionsHandler.onAcceptSuggestion(text)}
								/>
							{/if}
						{/snippet}
					</RichTextEditor>
				</div>
			</ConfigurableScrollableContainer>
		</div>

		<div class="message-textarea__toolbar">
			<EmojiPickerButton onEmojiSelect={(emoji) => onEmojiSelect(emoji.unicode)} />
			{#if enableFileUpload}
				<Button
					kind="ghost"
					icon="attachment-small"
					tooltip="Drop, paste or click to upload files"
					onclick={handleAttachFiles}
				/>
			{/if}
			<div class="message-textarea__toolbar__divider"></div>
			<Button
				kind="ghost"
				icon="slash-commands"
				tooltip="Slash commands"
				onclick={() => {
					// TODO: Implement slash commands
				}}
			/>
			<Button
				kind="ghost"
				icon="ai"
				tooltip={canUseAI
					? 'Generate message'
					: 'You need to enable AI in the project settings to use this feature'}
				disabled={!canUseAI}
				onclick={onAiButtonClick}
				loading={aiIsLoading}
			/>
			{#if !useRichText.current}
				<div class="message-textarea__toolbar__divider"></div>
				<FormattingButton
					icon="ruler"
					activated={useRuler.current}
					tooltip="Text ruler"
					onclick={() => {
						useRuler.current = !useRuler.current;
					}}
				/>
				<FormattingButton
					icon="text-wrap"
					disabled={!useRuler.current}
					activated={wrapTextByRuler.current && useRuler.current}
					tooltip="Wrap text automatically"
					onclick={() => {
						wrapTextByRuler.current = !wrapTextByRuler.current;
					}}
				/>
				<div class="message-textarea__ruler-input-wrapper" class:disabled={!useRuler.current}>
					<span class="text-13">Ruler:</span>
					<input
						disabled={!useRuler.current}
						value={rulerCountValue.current}
						min={MIN_RULER_VALUE}
						max={MAX_RULER_VALUE}
						class="text-13 text-input message-textarea__ruler-input"
						type="number"
						onfocus={() => (isEditorFocused = true)}
						onblur={() => {
							if (rulerCountValue.current < MIN_RULER_VALUE) {
								console.warn('Ruler value must be greater than 10');
								rulerCountValue.current = MIN_RULER_VALUE;
							} else if (rulerCountValue.current > MAX_RULER_VALUE) {
								rulerCountValue.current = MAX_RULER_VALUE;
							}

							isEditorFocused = false;
						}}
						oninput={(e) => {
							const input = e.currentTarget as HTMLInputElement;
							rulerCountValue.current = parseInt(input.value);
						}}
						onkeydown={(e) => {
							if (e.key === 'Enter') {
								e.preventDefault();
								composer?.focus();
							}
						}}
					/>
				</div>
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.editor-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		background-color: var(--clr-bg-1);
		overflow: auto;
		min-height: 0;
	}

	.editor-header {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.editor-tabs {
		z-index: var(--z-ground);
		position: relative;
		display: flex;
	}

	.editor-tab {
		position: relative;
		color: var(--clr-text-2);
		padding: 10px;
		background-color: var(--clr-bg-1-muted);
		border: 1px solid transparent;
		border-bottom: none;
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		transition:
			color var(--transition-fast),
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&.active {
			color: var(--clr-text-1);
			background-color: var(--clr-bg-1);
			border-color: var(--clr-border-2);

			&:after {
				content: '';
				position: absolute;
				bottom: 0;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-3);
				transform: translateY(100%);
			}
		}

		&.focused {
			border-color: var(--clr-border-1);
		}

		&:hover {
			color: var(--clr-text-1);
		}
	}

	/* MESSAGE INPUT */
	.message-textarea {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		border-radius: 0 var(--radius-m) var(--radius-m) var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
		min-height: 0;
		transition: border-color var(--transition-fast);

		&:hover,
		&:focus-within {
			border-color: var(--clr-border-1);
		}
	}

	.message-textarea__toolbar {
		flex: 0 0 auto;
		position: relative;
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 6px;
		padding: 0 12px;
		height: var(--lexical-input-client-toolbar-height);

		&:after {
			content: '';
			position: absolute;
			top: 0;
			left: 12px;
			width: calc(100% - 24px);
			height: 1px;
			background-color: var(--clr-border-3);
		}
	}

	.message-textarea__toolbar__divider {
		width: 1px;
		height: 18px;
		background-color: var(--clr-border-3);
	}

	/* RULER INPUT */
	.message-textarea__ruler-input-wrapper {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 0 4px;

		&.disabled {
			pointer-events: none;
			opacity: 0.5;
		}
	}

	.message-textarea__ruler-input {
		padding: 2px 0;
		width: 30px;
		text-align: center;

		/* remove numver arrows */
		&::-webkit-inner-spin-button,
		&::-webkit-outer-spin-button {
			-webkit-appearance: none;
			margin: 0;
		}
	}

	/*  */

	.message-textarea__inner {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-height: 0;
	}

	.message-textarea__wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
</style>
