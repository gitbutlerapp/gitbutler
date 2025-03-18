<script lang="ts">
	import { getEditor } from '../context';
	import { $getRoot as getRoot } from 'lexical';

	type OnChangeCallback = (value: string) => void;

	type Props = {
		onChange?: OnChangeCallback;
	};

	const { onChange }: Props = $props();

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
					text = getRoot().getTextContent();
					onChange?.(text);
				});
			}
		);
	});

	export function getText(): string | undefined {
		return text;
	}
</script>
