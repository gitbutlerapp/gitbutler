<script lang="ts" module>
	export type OnChangeCallback = (
		value: string,
		textUpToAnchor: string | undefined,
		textAfterAnchor: string | undefined
	) => void;
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getMarkdownString } from '$lib/richText/markdown';
	import { getEditorTextAfterAnchor, getEditorTextUpToAnchor } from '$lib/richText/selection';
	import {
		BLUR_COMMAND,
		COMMAND_PRIORITY_NORMAL,
		$getRoot as getRoot,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection
	} from 'lexical';
	import { untrack } from 'svelte';

	type Props = {
		markdown: boolean;
		maxLength?: number;
		onChange?: OnChangeCallback;
	};

	const { markdown, maxLength, onChange }: Props = $props();

	const editor = getEditor();

	let text = $state<string>();

	function getCurrentText() {
		// If WYSIWYG is enabled, we need to transform the content to markdown strings
		if (untrack(() => markdown)) return getMarkdownString(maxLength);
		return getRoot().getTextContent();
	}

	$effect(() => {
		return editor.registerCommand(
			BLUR_COMMAND,
			() => {
				editor.read(() => {
					const currentText = getCurrentText();
					if (currentText === text) {
						return;
					}

					text = currentText;
					const selection = getSelection();
					if (!isRangeSelection(selection)) {
						return;
					}

					const textUpToAnchor = getEditorTextUpToAnchor(selection);
					const textAfterAnchor = getEditorTextAfterAnchor(selection);
					onChange?.(text, textUpToAnchor, textAfterAnchor);
				});
				return false;
			},
			COMMAND_PRIORITY_NORMAL
		);
	});

	export function getText(): string | undefined {
		return text;
	}
</script>
