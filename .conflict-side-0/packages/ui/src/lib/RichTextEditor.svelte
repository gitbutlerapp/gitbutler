<script lang="ts">
	import { standardConfig } from '$lib/richText/config/config';
	import { standardTheme } from '$lib/richText/config/theme';
	import EmojiPlugin from '$lib/richText/plugins/Emoji.svelte';
	import MarkdownTransitionPlugin from '$lib/richText/plugins/markdownTransition.svelte';
	import OnChangePlugin, { type OnChangeCallback } from '$lib/richText/plugins/onChange.svelte';
	import { insertTextAtCaret, setEditorText } from '$lib/richText/selection';
	import {
		COMMAND_PRIORITY_CRITICAL,
		$getRoot as getRoot,
		KEY_DOWN_COMMAND,
		FOCUS_COMMAND,
		BLUR_COMMAND
	} from 'lexical';
	import { type Snippet } from 'svelte';
	import {
		Composer,
		ContentEditable,
		RichTextPlugin,
		SharedHistoryPlugin,
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
		LinkPlugin
	} from 'svelte-lexical';

	type Props = {
		namespace: string;
		markdown: boolean;
		onError: (error: unknown) => void;
		styleContext: 'client-editor' | 'chat-input';
		plugins?: Snippet;
		placeholder?: string;
		onFocus?: () => void;
		onBlur?: () => void;
		onChange?: OnChangeCallback;
		onKeyDown?: (event: KeyboardEvent | null) => boolean;
		initialText?: string;
		disabled?: boolean;
		wrapCountValue?: number;
	};

	const {
		disabled,
		namespace,
		markdown,
		onError,
		styleContext,
		plugins,
		placeholder,
		onFocus,
		onBlur,
		onChange,
		onKeyDown,
		initialText,
		wrapCountValue
	}: Props = $props();

	/** Standard configuration for our commit message editor. */
	const config = standardConfig({
		initialText,
		namespace,
		theme: standardTheme,
		onError
	});

	const isDisabled = $derived(disabled ?? false);

	const initialConfig = $derived({
		...config,
		editable: !isDisabled
	});
	/**
	 * Instance of the lexical composer, used for manipulating the contents of the editor
	 * programatically.
	 */
	let composer = $state<ReturnType<typeof Composer>>();

	let editorDiv: HTMLDivElement | undefined = $state();
	const editor = $derived(composer?.getEditor());

	let emojiPlugin = $state<ReturnType<typeof EmojiPlugin>>();

	// TODO: Change this plugin in favor of a toggle button.
	const markdownTransitionPlugin = new MarkdownTransitionPlugin(markdown);

	$effect(() => {
		if (editor) {
			markdownTransitionPlugin.setEditor(editor);
		}
	});

	$effect(() => {
		markdownTransitionPlugin.setMarkdown(markdown);
	});

	$effect(() => {
		if (editor) {
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

	export function getPlaintext(): Promise<string | undefined> {
		return new Promise((resolve) => {
			editor?.read(() => {
				const text = getRoot().getTextContent();
				resolve(text);
			});
		});
	}

	export function clear() {
		editor?.update(() => {
			const root = getRoot();
			root.clear();
		});
	}

	export function focus() {
		editor?.focus();
	}

	export function insertText(text: string) {
		focus();
		if (editor) {
			insertTextAtCaret(editor, text);
		}
	}

	export function setText(text: string) {
		if (editor) {
			setEditorText(editor, text);
		}
	}
</script>

<Composer {initialConfig} bind:this={composer}>
	<div
		class="lexical-container lexical-{styleContext}"
		bind:this={editorDiv}
		class:plain-text={!markdown}
	>
		<div class="editor-scroller scrollbar">
			<div class="editor">
				<ContentEditable />
				{#if placeholder}
					<PlaceHolder>{placeholder}</PlaceHolder>
				{/if}
			</div>
		</div>

		<EmojiPlugin bind:this={emojiPlugin} />
		<OnChangePlugin {markdown} {onChange} {wrapCountValue} />

		{#if markdown}
			<AutoFocusPlugin />
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
			<SharedHistoryPlugin />
		{:else}
			<PlainTextPlugin />
		{/if}

		{#if plugins}
			{@render plugins()}
		{/if}
	</div>
</Composer>

<style lang="postcss">
	.lexical-container {
		flex-grow: 1;
		background-color: var(--clr-bg-1);
		position: relative;
		display: block;
	}

	.editor-scroller {
		border: 0;
		display: flex;
		flex-direction: column;
		position: relative;
		outline: 0;
		z-index: 0;
		overflow: auto;
		height: 100%;
		/* It's unclear why the resizer is on by default on this element. */
		resize: none;
	}

	.editor {
		flex: auto;
		position: relative;
		resize: vertical;
		z-index: -1;
	}
</style>
