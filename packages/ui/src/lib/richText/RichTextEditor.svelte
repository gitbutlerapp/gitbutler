<script lang="ts">
	import { focusable } from '$lib/focus/focusable';
	import { WRAP_ALL_COMMAND } from '$lib/richText/commands';
	import { standardConfig } from '$lib/richText/config/config';
	import { standardTheme } from '$lib/richText/config/theme';
	import { INLINE_CODE_TRANSFORMER } from '$lib/richText/customTransforers';
	import { getCurrentText } from '$lib/richText/getText';
	// import CodeBlockTypeAhead from '$lib/richText/plugins/CodeBlockTypeAhead.svelte';
	import EmojiPlugin from '$lib/richText/plugins/Emoji.svelte';
	import PlainTextIndentPlugin from '$lib/richText/plugins/PlainTextIndentPlugin.svelte';
	import MarkdownTransitionPlugin from '$lib/richText/plugins/markdownTransition';
	import OnChangePlugin, { type OnChangeCallback } from '$lib/richText/plugins/onChange.svelte';
	import OnInput, { type OnInputCallback } from '$lib/richText/plugins/onInput.svelte';
	import { insertTextAtCaret, setEditorText } from '$lib/richText/selection';
	import {
		COMMAND_PRIORITY_CRITICAL,
		$getRoot as getRoot,
		$isParagraphNode as isParagraphNode,
		KEY_DOWN_COMMAND,
		FOCUS_COMMAND,
		BLUR_COMMAND,
		type SerializedEditorState
	} from 'lexical';
	import { type Snippet } from 'svelte';
	import {
		Composer,
		ContentEditable,
		RichTextPlugin,
		ListPlugin,
		CheckListPlugin,
		AutoFocusPlugin,
		PlaceHolder,
		HashtagPlugin,
		PlainTextPlugin,
		AutoLinkPlugin,
		FloatingLinkEditorPlugin,
		CodeHighlightPlugin,
		CodeActionMenuPlugin,
		MarkdownShortcutPlugin,
		ALL_TRANSFORMERS,
		LinkPlugin,
		HistoryPlugin
	} from 'svelte-lexical';

	interface Props {
		namespace: string;
		plaintext: boolean;
		onError: (error: unknown) => void;
		styleContext: 'client-editor' | 'chat-input';
		plugins?: Snippet;
		placeholder?: string;
		minHeight?: string;
		maxHeight?: string;
		value?: string;
		onFocus?: () => void;
		onBlur?: () => void;
		onChange?: OnChangeCallback;
		onInput?: OnInputCallback;
		onKeyDown?: (event: KeyboardEvent | null) => boolean;
		initialText?: string;
		disabled?: boolean;
		wrapCountValue?: number;
		useMonospaceFont?: boolean;
		monospaceFont?: string;
		tabSize?: number;
		enableLigatures?: boolean;
		autoFocus?: boolean;
	}

	let {
		disabled,
		namespace,
		plaintext,
		onError,
		minHeight,
		maxHeight,
		styleContext,
		plugins,
		placeholder,
		value = $bindable(),
		onFocus,
		onBlur,
		onChange,
		onInput,
		onKeyDown,
		initialText,
		wrapCountValue,
		useMonospaceFont,
		monospaceFont,
		tabSize,
		enableLigatures,
		autoFocus = true
	}: Props = $props();

	/** Standard configuration for our commit message editor. */
	const initialConfig = $derived(
		standardConfig({
			namespace,
			theme: standardTheme,
			onError
		})
	);

	/**
	 * Instance of the lexical composer, used for manipulating the contents of the editor
	 * programatically.
	 */
	let composer = $state<ReturnType<typeof Composer>>();
	let editorDiv: HTMLDivElement | undefined = $state();
	let emojiPlugin = $state<ReturnType<typeof EmojiPlugin>>();

	// TODO: Change this plugin in favor of a toggle button.
	const markdownTransitionPlugin = new MarkdownTransitionPlugin(wrapCountValue);

	const isDisabled = $derived(disabled ?? false);

	$effect(() => {
		if (composer) {
			const editor = composer.getEditor();
			if (isDisabled && editor.isEditable()) {
				editor.setEditable(false);
			} else if (!isDisabled && !editor.isEditable()) {
				editor.setEditable(true);
			}
		}
	});

	$effect(() => {
		if (composer) {
			const editor = composer.getEditor();
			markdownTransitionPlugin.setEditor(editor);
		}
	});

	$effect(() => {
		markdownTransitionPlugin.setMarkdown(!plaintext);
	});

	$effect(() => {
		if (wrapCountValue) {
			markdownTransitionPlugin.setMaxLength(wrapCountValue);
		}
	});

	$effect(() => {
		if (composer) {
			const editor = composer.getEditor();
			const unregidterKeyDown = editor.registerCommand<KeyboardEvent | null>(
				KEY_DOWN_COMMAND,
				(e) => {
					if (emojiPlugin?.isBusy()) {
						return false;
					}
					return onKeyDown?.(e) ?? false;
				},
				COMMAND_PRIORITY_CRITICAL
			);
			const unregisterFocus = editor.registerCommand(
				FOCUS_COMMAND,
				() => {
					onFocus?.();
					return false;
				},
				COMMAND_PRIORITY_CRITICAL
			);
			const unregisterBlur = editor.registerCommand(
				BLUR_COMMAND,
				() => {
					onBlur?.();
					return false;
				},
				COMMAND_PRIORITY_CRITICAL
			);

			return () => {
				unregidterKeyDown();
				unregisterFocus();
				unregisterBlur();
			};
		}
	});

	// Initial text is available asynchronously so we need to be able to
	// insert initial text after first render.
	$effect(() => {
		updateInitialtext(initialText);
	});

	async function updateInitialtext(initialText: string | undefined) {
		if (initialText) {
			const currentText = await getPlaintext();
			if (currentText?.trim() === '') {
				setText(initialText);
			}
		}
	}

	export function getPlaintext(): Promise<string | undefined> {
		return new Promise((resolve) => {
			if (composer) {
				const editor = composer.getEditor();
				editor?.read(() => {
					const text = getCurrentText(!plaintext, wrapCountValue);
					resolve(text);
				});
			}
		});
	}

	export function getParagraphCount(): Promise<number> {
		return new Promise((resolve) => {
			if (!composer) {
				resolve(0);
				return;
			}
			composer.getEditor()?.read(() => {
				const root = getRoot();
				const count = root.getChildren().filter(isParagraphNode).length;
				resolve(count);
			});
		});
	}

	export function clear() {
		if (!composer) {
			return;
		}
		const editor = composer.getEditor();
		editor?.update(() => {
			const root = getRoot();
			root.clear();
		});
	}

	export function focus() {
		if (!composer) {
			return;
		}
		const editor = composer.getEditor();
		// We should be able to use `editor.focus()` here, but for some reason
		// it only works after the input has already been focused.
		editor.getRootElement()?.focus();
	}

	export function wrapAll() {
		const editor = composer?.getEditor();
		if (editor) {
			editor.dispatchCommand(WRAP_ALL_COMMAND, undefined);
		}
	}

	export function insertText(text: string) {
		if (!composer) {
			return;
		}
		focus();
		const editor = composer.getEditor();
		insertTextAtCaret(editor, text);
	}

	export function setText(text: string) {
		if (!composer) return;
		const editor = composer.getEditor();
		setEditorText(editor, text);
	}

	export function save() {
		return composer?.getEditor().getEditorState().toJSON();
	}

	export function load(state: SerializedEditorState) {
		const editor = composer?.getEditor();
		const editorState = editor?.parseEditorState(state);
		if (editorState) {
			editor?.setEditorState(editorState);
		}
	}
</script>

<Composer {initialConfig} bind:this={composer}>
	<div
		class="lexical-container lexical-{styleContext} scrollbar"
		bind:this={editorDiv}
		use:focusable={{ button: true }}
		class:plain-text={plaintext}
		class:disabled={isDisabled}
		style:min-height={minHeight}
		style:max-height={maxHeight}
		style:--code-block-font={useMonospaceFont && monospaceFont
			? monospaceFont
			: 'var(--font-default)'}
		style:--code-block-tab-size={useMonospaceFont && tabSize ? tabSize : 4}
		style:--code-block-ligatures={useMonospaceFont && enableLigatures
			? 'common-ligatures'
			: 'normal'}
		style:--lexical-input-client-text-wrap={useMonospaceFont ? 'nowrap' : 'normal'}
	>
		<div class="editor">
			<ContentEditable />
			{#if placeholder}
				<PlaceHolder>{placeholder}</PlaceHolder>
			{/if}
		</div>

		<EmojiPlugin bind:this={emojiPlugin} />

		<OnChangePlugin
			markdown={!plaintext}
			onChange={(newValue, changeUpToAnchor, textAfterAnchor) => {
				value = newValue;
				onChange?.(newValue, changeUpToAnchor, textAfterAnchor);
			}}
			maxLength={wrapCountValue}
		/>

		{#if onInput}
			<OnInput markdown={!plaintext} {onInput} maxLength={wrapCountValue} />
		{/if}

		{#if plaintext}
			<PlainTextPlugin />
			<PlainTextIndentPlugin />
			<!-- <CodeBlockTypeAhead /> -->
			<MarkdownShortcutPlugin transformers={[INLINE_CODE_TRANSFORMER]} />
		{:else}
			<AutoLinkPlugin />
			<CheckListPlugin />
			<CodeActionMenuPlugin anchorElem={editorDiv} />
			<CodeHighlightPlugin />
			<FloatingLinkEditorPlugin anchorElem={editorDiv} />
			<HashtagPlugin />
			<ListPlugin />
			<LinkPlugin />
			<MarkdownShortcutPlugin transformers={ALL_TRANSFORMERS} />
			<RichTextPlugin />
		{/if}
		{#if autoFocus}
			<AutoFocusPlugin />
		{/if}
		<HistoryPlugin />

		{#if plugins}
			{@render plugins()}
		{/if}
	</div>
</Composer>

<style lang="postcss">
	.lexical-container {
		display: block;
		z-index: 0;
		position: relative;
		flex-grow: 1;
		overflow: auto;
		background-color: var(--clr-bg-1);
	}

	.editor-scroller {
		display: flex;
		z-index: 0;
		position: relative;
		flex-direction: column;
		height: 100%;
		overflow: auto;
		border: 0;
		outline: 0;
		/* It's unclear why the resizer is on by default on this element. */
		resize: none;
	}

	.editor {
		z-index: -1;
		position: relative;
		flex: auto;
		resize: vertical;
	}

	.disabled {
		opacity: 0.5;
		pointer-events: none;
	}
</style>
