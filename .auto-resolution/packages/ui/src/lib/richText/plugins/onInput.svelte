<script lang="ts" module>
	export type OnInputCallback = (
		value: string,
		textUpToAnchor: string | undefined,
		textAfterAnchor: string | undefined
	) => void;
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getEditorTextAfterAnchor, getEditorTextUpToAnchor } from '$lib/richText/selection';
	import { exportPlaintext } from '$lib/richText/utils/export';
	import {
		$getRoot as getRoot,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection
	} from 'lexical';

	type Props = {
		onInput?: OnInputCallback;
	};

	const { onInput }: Props = $props();

	const editor = getEditor();

	let text = $state<string>();

	$effect(() => {
		return editor.registerUpdateListener(
			({ editorState, dirtyElements, dirtyLeaves, prevEditorState, tags }) => {
				if (
					tags.has('history-merge') ||
					(dirtyElements.size === 0 && dirtyLeaves.size === 0) ||
					prevEditorState.isEmpty()
				) {
					return;
				}

				editorState.read(() => {
					const currentText = exportPlaintext(getRoot());
					if (currentText === text) return;
					text = currentText;

					const selection = getSelection();
					if (!isRangeSelection(selection)) {
						return;
					}

					const textUpToAnchor = getEditorTextUpToAnchor(selection);
					const textAfterAnchor = getEditorTextAfterAnchor(selection);
					onInput?.(text, textUpToAnchor, textAfterAnchor);
				});
			}
		);
	});

	export function getText(): string | undefined {
		return text;
	}
</script>
