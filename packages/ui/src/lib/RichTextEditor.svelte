<script lang="ts">
	import MarkdownTransitionPlugin from './richText/plugins/markdownTransition.svelte';
	import { standardConfig } from '$lib/richText/config/config';
	import { standardTheme } from '$lib/richText/config/theme';
	import EmojiPlugin from '$lib/richText/plugins/Emoji.svelte';
	import OnChangePlugin from '$lib/richText/plugins/onChange.svelte';
	import OnKeyDownPlugin from '$lib/richText/plugins/onKeyDown.svelte';
	import { $getRoot as getRoot } from 'lexical';
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
		Toolbar,
		StateStoreRichTextUpdator,
		LinkPlugin
	} from 'svelte-lexical';

	type Props = {
		namespace: string;
		markdown: boolean;
		onError: (error: unknown) => void;
		toolBar?: Snippet;
		plugins?: Snippet;
		placeholder?: string;
		onChange?: (text: string) => void;
		onKeyDown?: (event: KeyboardEvent) => void;
	};

	const {
		namespace,
		markdown,
		onError,
		toolBar,
		plugins,
		placeholder,
		onChange,
		onKeyDown
	}: Props = $props();

	/** Standard configuration for our commit message editor. */
	const initialConfig = standardConfig({
		namespace,
		theme: standardTheme,
		onError
	});

	/**
	 * Instance of the lexical composer, used for manipulating the contents of the editor
	 * programatically.
	 */
	let composer = $state<ReturnType<typeof Composer>>();

	let editorDiv: HTMLDivElement | undefined = $state();
	const editor = $derived(composer?.getEditor());

	let onChangeRef = $state<ReturnType<typeof OnChangePlugin>>();

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
</script>

<Composer {initialConfig} bind:this={composer}>
	{#if toolBar}
		<Toolbar>
			<StateStoreRichTextUpdator />
			{@render toolBar()}
		</Toolbar>
	{/if}

	<div class="editor-container" bind:this={editorDiv}>
		<div class="editor-scroller">
			<div class="editor">
				<ContentEditable />
				{#if placeholder}
					<PlaceHolder>{placeholder}</PlaceHolder>
				{/if}
			</div>
		</div>

		<EmojiPlugin />
		<OnChangePlugin bind:this={onChangeRef} {onChange} />
		<OnKeyDownPlugin {onKeyDown} />

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
			{#if plugins}
				{@render plugins()}
			{/if}
		{:else}
			<PlainTextPlugin />
		{/if}
	</div>
</Composer>

<style>
	.editor-container {
		flex-grow: 1;
		background-color: var(--clr-bg-1);
		position: relative;
		display: block;
	}

	.editor-scroller {
		height: 100%;
		/* It's unclear why the resizer is on by default on this element. */
		resize: none;
	}
</style>
