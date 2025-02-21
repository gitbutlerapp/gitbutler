<script lang="ts">
	import { showError } from '$lib/notifications/toasts';
	import { standardConfig } from '$lib/textEditor/config/config';
	import { standardTheme } from '$lib/textEditor/config/theme';
	import { emojiTextNodeTransform } from '$lib/textEditor/plugins/emojiPlugin';
	import {
		$convertToMarkdownString as convertToMarkdownString,
		$convertFromMarkdownString as convertFromMarkdownString
	} from '@lexical/markdown';
	import {
		$createParagraphNode as createParagraphNode,
		$createTextNode as createTextNode,
		$getRoot as getRoot,
		TextNode
	} from 'lexical';
	import { onMount } from 'svelte';
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
		ALL_TRANSFORMERS
	} from 'svelte-lexical';

	type Props = {
		markdown: boolean;
	};

	const { markdown = $bindable() }: Props = $props();

	/**
	 * The stackId parameter is currently optional, mainly so that we don't
	 *
	 * TODO: Figure out if we can show markdown rendered placeholder text.
	 */
	const placeholder = 'Your commit summary';

	/**
	 * Instance of the lexical composer, used for manipulating the contents of the editor
	 * programatically.
	 */
	let composer: Composer;

	/** Standard configuration for our commit message editor. */
	const initialConfig = standardConfig({
		theme: standardTheme,
		onError: (error: unknown) => {
			showError('Editor error', error);
		}
	});

	let editorDiv: HTMLDivElement | undefined = $state();

	onMount(() => {
		const unlistenEmoji = composer
			.getEditor()
			.registerNodeTransform(TextNode, emojiTextNodeTransform);
		return () => {
			unlistenEmoji();
		};
	});

	$effect(() => {
		const editor = composer.getEditor();
		if (markdown) {
			editor.update(() => {
				convertFromMarkdownString(getRoot().getTextContent(), ALL_TRANSFORMERS);
			});
		} else {
			getPlaintext((text) => {
				editor.update(() => {
					const root = getRoot();
					root.clear();
					const paragraph = createParagraphNode();
					paragraph.append(createTextNode(text));
					root.append(paragraph);
				});
			});
		}
	});

	export function getPlaintext(callback: (text: string) => void) {
		const editor = composer.getEditor();
		const state = editor.getEditorState();
		state.read(() => {
			const markdown = convertToMarkdownString(ALL_TRANSFORMERS);
			callback(markdown);
		});
	}
</script>

<Composer {initialConfig} bind:this={composer}>
	<div class="editor-container" bind:this={editorDiv}>
		<div class="editor-scroller">
			<div class="editor">
				<ContentEditable />
				<PlaceHolder>{placeholder}</PlaceHolder>
			</div>
		</div>

		{#if markdown}
			<AutoFocusPlugin />
			<AutoLinkPlugin />
			<CheckListPlugin />
			<CodeActionMenuPlugin anchorElem={editorDiv} />
			<CodeHighlightPlugin />
			<FloatingLinkEditorPlugin anchorElem={editorDiv} />
			<HashtagPlugin />
			<ListPlugin />
			<MarkdownShortcutPlugin transformers={ALL_TRANSFORMERS} />
			<RichTextPlugin />
			<SharedHistoryPlugin />
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
