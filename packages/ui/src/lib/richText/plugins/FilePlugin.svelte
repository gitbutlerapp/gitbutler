<script lang="ts">
	import { debounce } from '$lib/utils/debounce';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
	import {
		$isRangeSelection as isRangeSelection,
		$getSelection as getSelection,
		TextNode,
		SELECTION_CHANGE_COMMAND,
		COMMAND_PRIORITY_NORMAL,
		$isTextNode as isTextNode
	} from 'lexical';
	import { onMount } from 'svelte';
	import { getEditor } from 'svelte-lexical';

	type Props = {
		onQuery: (query: string, callback?: (result: string) => void) => void;
	};

	const { onQuery }: Props = $props();

	const editor = getEditor();
	const FILEPATH_REGEX = /@([^ ]+)$/i;

	let nodeKey = '';

	onMount(() => {
		return mergeUnlisten(
			editor.registerMutationListener(TextNode, () => {
				editor.read(() => {
					const selection = getSelection();
					if (!isRangeSelection(selection)) return;
					if (!selection.isCollapsed()) return;

					const anchor = selection.anchor;
					const anchorNode = anchor.getNode();
					if (!isTextNode(anchorNode)) return;

					const offset = anchor.offset;
					const text = anchorNode.getTextContent().slice(0, offset);
					const match = text.match(FILEPATH_REGEX);

					if (match) {
						const query = match[1];
						nodeKey = selection.anchor.getNode().getKey();
						debouncedOnQuery(query, (result) => handleCallback(query, result));
					} else {
						onQuery('');
					}
				});
			}),
			editor.registerCommand(
				SELECTION_CHANGE_COMMAND,
				() => {
					const selection = getSelection();
					if (isRangeSelection(selection)) {
						const anchorKey = selection.anchor.getNode().getKey();
						if (nodeKey !== anchorKey) {
							onQuery('');
						}
					}
					return false;
				},
				COMMAND_PRIORITY_NORMAL
			)
		);
	});

	const debouncedOnQuery = debounce(onQuery, 100);

	function handleCallback(query: string, path: string) {
		editor.update(() => {
			const selection = getSelection();
			if (isRangeSelection(selection)) {
				const node = selection.anchor.getNode();
				const text = node.getTextContent();
				const match = '@' + query;
				if (text.includes(match)) {
					const newText = text.replace(match, '`' + path + '`');
					const newNode = new TextNode(newText);
					node.replace(newNode);
					newNode.selectEnd();
				}
				onQuery('');
			}
		});
	}
</script>
