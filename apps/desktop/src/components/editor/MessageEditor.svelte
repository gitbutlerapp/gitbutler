<script lang="ts" module>
	export type AiButtonClickParams = {
		useEmojiStyle?: boolean;
		useBriefStyle?: boolean;
	};
</script>

<script lang="ts">
	import MessageEditorRuler from '$components/editor/MessageEditorRuler.svelte';
	import CommitSuggestions from '$components/editor/commitSuggestions.svelte';
	import {
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis
	} from '$lib/config/config';
	import { showError } from '$lib/notifications/toasts';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { uploadFiles } from '@gitbutler/shared/dom';
	import { persisted } from '@gitbutler/shared/persisted';
	import { UPLOADS_SERVICE } from '@gitbutler/shared/uploads/uploadsService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import EmojiPickerButton from '@gitbutler/ui/EmojiPickerButton.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import FileUploadPlugin, {
		type DropFileResult
	} from '@gitbutler/ui/richText/plugins/FileUpload.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
	import HardWrapPlugin from '@gitbutler/ui/richText/plugins/HardWrapPlugin.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';
	import FormattingButton from '@gitbutler/ui/richText/tools/FormattingButton.svelte';

	import { tick } from 'svelte';

	const ACCEPTED_FILE_TYPES = ['image/*', 'application/*', 'text/*', 'audio/*', 'video/*'];

	interface Props {
		projectId: string;
		disabled?: boolean;
		initialValue?: string;
		placeholder: string;
		onChange?: (text: string) => void;
		onKeyDown?: (e: KeyboardEvent) => boolean;
		enableFileUpload?: boolean;
		enableSmiles?: boolean;
		enableRuler?: boolean;
		onAiButtonClick: (params: AiButtonClickParams) => void;
		canUseAI: boolean;
		aiIsLoading: boolean;
		suggestionsHandler?: CommitSuggestions;
		testId?: string;
		forceSansFont?: boolean;
	}

	let {
		projectId,
		initialValue,
		placeholder,
		disabled,
		enableFileUpload,
		enableSmiles,
		onChange,
		onKeyDown,
		onAiButtonClick,
		enableRuler,
		canUseAI,
		aiIsLoading,
		suggestionsHandler,
		testId,
		forceSansFont
	}: Props = $props();

	const MIN_RULER_VALUE = 30;
	const MAX_RULER_VALUE = 200;

	const uiState = inject(UI_STATE);

	const uploadsService = inject(UPLOADS_SERVICE);
	const userSettings = inject(SETTINGS);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);

	const useFloatingBox = uiState.global.useFloatingBox;

	const rulerCountValue = uiState.global.rulerCountValue;
	const useRuler = uiState.global.useRuler;

	const wrapCountValue = $derived(rulerCountValue.current);

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();
	let fileUploadPlugin = $state<ReturnType<typeof FileUploadPlugin>>();
	let uploadConfirmationModal = $state<ReturnType<typeof Modal>>();
	const doNotShowUploadWarning = persisted<boolean>(false, 'doNotShowUploadWarning');
	let allowUploadOnce = $state<boolean>(false);
	let uploadedBy = $state<'drop' | 'attach' | undefined>(undefined);
	let tempDropFiles: FileList | undefined = $state(undefined);

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

	function handleKeyDown(event: KeyboardEvent | null) {
		if (event && !event.metaKey && !event.ctrlKey) {
			// Prevent regular keystrokes from propagating so that we don't
			// trigger keyboard shortcuts while the user is typing a message.
			event.stopPropagation();
		}
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

		uploadedBy = 'drop';
		tempDropFiles = files;
		uploadConfirmationModal?.show();
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
		uploadedBy = 'attach';
		uploadConfirmationModal?.show();
	}

	export function focus() {
		composer?.focus();
	}

	export function setText(text: string) {
		composer?.setText(text);
	}

	export function isRichTextMode(): boolean {
		return false;
	}

	// We want to avoid letting most mouse events bubble up to the parent.
	function stopPropagation(e: MouseEvent) {
		e.stopPropagation();
	}

	function handleGenerateMessage() {
		if (aiIsLoading) return;

		onAiButtonClick({
			useEmojiStyle: $commitGenerationUseEmojis,
			useBriefStyle: $commitGenerationExtraConcise
		});
	}

	const DROPDOWN_BTN_BREAKPOINTS = {
		short: 280,
		medium: 320
	};

	function getTooltipText(): string | undefined {
		if (!canUseAI) {
			return 'You need to enable AI in the project settings to use this feature';
		}
		if (currentEditorWidth <= DROPDOWN_BTN_BREAKPOINTS.medium) {
			return 'Generate commit message';
		}
		return undefined;
	}

	let currentEditorWidth = $state<number>(0);
</script>

{#snippet buttonText()}
	{currentEditorWidth > DROPDOWN_BTN_BREAKPOINTS.medium ? 'Generate message' : 'Generate'}
{/snippet}

<Modal
	type="warning"
	title="Off to the cloud it goes!"
	width="small"
	bind:this={uploadConfirmationModal}
	onSubmit={async (close) => {
		allowUploadOnce = true;

		if (uploadedBy === 'drop') {
			const files = tempDropFiles;
			tempDropFiles = undefined;
			if (files) {
				composer?.focus();
				await fileUploadPlugin?.handleFileUpload(files);
			}
		} else if (uploadedBy === 'attach') {
			await attachFiles();
		}

		uploadedBy = undefined;
		close();
	}}
>
	Your file will be stored in GitButler’s digital vault, safe and sound. We promise it’s secure, so
	feel free to share the link however you like 🔐
	{#snippet controls(close)}
		<div class="modal-footer">
			<div class="flex flex-1">
				<label for="dont-show-again" class="modal-footer__checkbox">
					<Checkbox name="dont-show-again" small bind:checked={$doNotShowUploadWarning} />
					<span class="text-12"> Don’t show again</span>
				</label>
			</div>
			<Button kind="outline" onclick={close}>Cancel</Button>
			<Button style="pop" type="submit">Yes, upload!</Button>
		</div>
	{/snippet}
</Modal>

<div
	data-remove-from-panning
	role="presentation"
	class="editor-wrapper hide-native-scrollbar"
	style:--lexical-input-client-text-wrap={useRuler.current && !forceSansFont ? 'nowrap' : 'normal'}
	style:--extratoolbar-height={useFloatingBox.current ? '2.625rem' : '0'}
	style:--code-block-font={useRuler.current && !forceSansFont
		? $userSettings.diffFont
		: 'var(--fontfamily-default)'}
	style:--code-block-tab-size={$userSettings.tabSize && !forceSansFont ? $userSettings.tabSize : 4}
	style:--code-block-ligatures={$userSettings.diffLigatures && !forceSansFont
		? 'common-ligatures'
		: 'normal'}
	onclick={stopPropagation}
	ondblclick={stopPropagation}
	onmousedown={stopPropagation}
	onmouseup={stopPropagation}
	ondrag={stopPropagation}
	ondragend={stopPropagation}
	ondragenter={stopPropagation}
	ondragexit={stopPropagation}
	ondragleave={stopPropagation}
	ondragover={stopPropagation}
	ondragstart={stopPropagation}
	ondrop={stopPropagation}
>
	<div role="presentation" class="message-textarea">
		<div
			bind:clientWidth={currentEditorWidth}
			data-testid={testId}
			role="presentation"
			class="message-textarea__inner"
			onclick={() => {
				composer?.focus();
			}}
		>
			{#if useRuler.current && enableRuler}
				<MessageEditorRuler />
			{/if}

			{#if useFloatingBox.current}
				<div class="editor-extratools">
					<FormattingBar {formatter} />
				</div>
			{/if}

			<div class="message-textarea__wrapper">
				<RichTextEditor
					minHeight="4rem"
					styleContext="client-editor"
					namespace="CommitMessageEditor"
					{placeholder}
					bind:this={composer}
					markdown={false}
					onError={(e) => console.warn('Editor error', e)}
					initialText={initialValue}
					onChange={handleChange}
					onKeyDown={handleKeyDown}
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
						<HardWrapPlugin enabled={useRuler.current} maxLength={wrapCountValue} />
					{/snippet}
				</RichTextEditor>
			</div>
		</div>

		<div class="message-textarea__toolbar">
			<div class="message-textarea__toolbar__left">
				<Button
					kind="ghost"
					icon={useFloatingBox.current ? 'exit-floating-box' : 'enter-floating-box'}
					tooltip={useFloatingBox.current ? 'Exit floating mode' : 'Use floating mode'}
					onclick={() => {
						useFloatingBox.set(!useFloatingBox.current);
					}}
				/>
				<div class="message-textarea__toolbar__divider"></div>
				{#if enableSmiles}
					<EmojiPickerButton onEmojiSelect={(emoji) => onEmojiSelect(emoji.unicode)} />
				{/if}
				{#if enableFileUpload}
					<Button
						kind="ghost"
						icon="attachment-small"
						tooltip="Drop, paste or click to upload files"
						onclick={handleAttachFiles}
					/>
				{/if}
				{#if enableRuler}
					<FormattingButton
						icon="text-wrap"
						activated={useRuler.current}
						tooltip="Wrap text automatically"
						onclick={async () => {
							useRuler.set(!useRuler.current);
							await tick(); // Wait for reactive update.
							if (useRuler.current) {
								composer?.wrapAll();
							}
						}}
					/>
					{#if useRuler.current}
						<div class="message-textarea__ruler-input-wrapper">
							<input
								value={rulerCountValue.current}
								min={MIN_RULER_VALUE}
								max={MAX_RULER_VALUE}
								class="text-13 text-input message-textarea__ruler-input"
								type="number"
								onblur={() => {
									if (rulerCountValue.current < MIN_RULER_VALUE) {
										console.warn('Ruler value must be greater than 10');
										rulerCountValue.set(MIN_RULER_VALUE);
									} else if (rulerCountValue.current > MAX_RULER_VALUE) {
										rulerCountValue.set(MAX_RULER_VALUE);
									}
								}}
								oninput={(e) => {
									const input = e.currentTarget as HTMLInputElement;
									rulerCountValue.set(parseInt(input.value));
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
				{/if}
			</div>
			<DropDownButton
				kind="outline"
				icon="ai-small"
				shrinkable
				disabled={!canUseAI}
				loading={aiIsLoading}
				menuPosition="top"
				onclick={handleGenerateMessage}
				children={currentEditorWidth > DROPDOWN_BTN_BREAKPOINTS.short ? buttonText : undefined}
				tooltip={getTooltipText()}
			>
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Extra concise"
							onclick={() => ($commitGenerationExtraConcise = !$commitGenerationExtraConcise)}
						>
							{#snippet control()}
								<Checkbox small bind:checked={$commitGenerationExtraConcise} />
							{/snippet}
						</ContextMenuItem>

						<ContextMenuItem
							label="Use emojis 😎"
							onclick={() => ($commitGenerationUseEmojis = !$commitGenerationUseEmojis)}
						>
							{#snippet control()}
								<Checkbox small bind:checked={$commitGenerationUseEmojis} />
							{/snippet}
						</ContextMenuItem>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		</div>
	</div>
</div>

<style lang="postcss">
	.editor-wrapper {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
		overflow: auto;
	}

	.editor-extratools {
		display: flex;
		align-items: center;
		height: var(--extratoolbar-height);
		padding: 0 10px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-2);
	}

	/* MESSAGE INPUT */
	.message-textarea {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		background-color: var(--clr-bg-1);
		transition: border-color var(--transition-fast);

		&:hover,
		&:focus-within {
			border-color: var(--clr-border-1);
		}
	}

	.message-textarea__toolbar {
		container-type: inline-size;
		display: flex;
		position: relative;
		flex: 0 0 auto;
		align-items: center;
		justify-content: flex-start;
		height: var(--lexical-input-client-toolbar-height);
		padding: 0 8px;
		gap: 6px;

		&:after {
			position: absolute;
			top: 0;
			left: 8px;
			width: calc(100% - 16px);
			height: 1px;
			background-color: var(--clr-border-3);
			content: '';
		}
	}

	.message-textarea__toolbar__left {
		display: flex;
		flex: 1;
		align-items: center;
		gap: 6px;
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
		padding: 0 4px;
		gap: 5px;
	}

	.message-textarea__ruler-input {
		width: 30px;
		padding: 2px 0;
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
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}

	.message-textarea__wrapper {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	/* MODAL */
	.modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.modal-footer__checkbox {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
