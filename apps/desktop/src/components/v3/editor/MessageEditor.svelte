<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { debouncePromise } from '@gitbutler/shared/utils/misc';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmojiPickerButton from '@gitbutler/ui/EmojiPickerButton.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';

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

	const uiState = getContext(UiState);
	const useRichText = uiState.global.useRichText;

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();
	let isEditorHovered = $state(false);
	let isEditorFocused = $state(false);

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

	export function focus() {
		composer?.focus();
	}

	export function setText(text: string) {
		composer?.setText(text);
	}
</script>

<div class="editor-wrapper">
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
				}}>Rich-text Editor</button
			>
		</div>
		<FormattingBar bind:formatter {onAiButtonClick} {canUseAI} aiLoading={aiIsLoading} />
	</div>

	<div
		role="presentation"
		class="message-editor"
		onmouseenter={() => (isEditorHovered = true)}
		onmouseleave={() => (isEditorHovered = false)}
		onclick={() => {
			composer?.focus();
		}}
	>
		<div class="message-editor__inner">
			<ConfigurableScrollableContainer height="100%">
				<div class="message-editor__wrapper">
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
					>
						{#snippet plugins()}
							<Formatter bind:this={formatter} />
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

		<div class="message-editor__inner-toolbar">
			<EmojiPickerButton onEmojiSelect={(emoji) => onEmojiSelect(emoji.unicode)} />
			{#if enableFileUpload}
				<div class="message-editor__inner-toolbar__divider"></div>
				<Button kind="ghost" icon="attachment-small" reversedDirection>
					<span style="opacity: 0.4">Drop or click to add files</span>
				</Button>
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

	.message-editor {
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

	.message-editor__inner-toolbar {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 6px;
		padding: 10px 12px;

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

	.message-editor__inner-toolbar__divider {
		width: 1px;
		height: 18px;
		background-color: var(--clr-border-3);
	}

	.message-editor__inner {
		flex: 1;
		display: flex;
		flex-direction: column;

		overflow: hidden;
		min-height: 0;
	}

	.message-editor__wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
</style>
