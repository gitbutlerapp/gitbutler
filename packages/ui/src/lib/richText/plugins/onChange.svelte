<script lang="ts" module>
	export type OnChangeCallback = (
		value: string,
		textUpToAnchor: string | undefined,
		textAfterAnchor: string | undefined
	) => void;
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getEditorTextAfterAnchor, getEditorTextUpToAnchor } from '$lib/richText/selection';
	import { $getRoot as getRoot } from 'lexical';
	import {
		BLUR_COMMAND,
		COMMAND_PRIORITY_NORMAL,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection
	} from 'lexical';

	type Props = {
		onChange?: OnChangeCallback;
	};

	const { onChange }: Props = $props();

	const editor = getEditor();

	let text = $state<string>();

	$effect(() => {
		return editor.registerCommand(
			BLUR_COMMAND,
			() => {
				editor.read(() => {
					const currentText = getRoot().getTextContent();
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
