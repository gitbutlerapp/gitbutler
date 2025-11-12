<script lang="ts" module>
	export type FileSuggestionUpdate =
		| {
				loading: false;
				items: string[];
		  }
		| {
				loading: true;
		  };
</script>

<script lang="ts">
	import { createInlineCodeNode } from '$lib/richText/node/inlineCode';
	import TypeAhead from '$lib/richText/plugins/TypeAhead.svelte';
	import {
		$isRangeSelection as isRangeSelection,
		$getSelection as getSelection,
		TextNode,
		$isTextNode as isTextNode
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	type Props = {
		getFileItems: (q: string) => Promise<string[]>;
		onUpdateSuggestion: (p: FileSuggestionUpdate, query: string) => void;
		onExitSuggestion: () => void;
	};

	const { onExitSuggestion, getFileItems, onUpdateSuggestion }: Props = $props();

	type FileMatch = {
		matchText: string;
		captureText: string;
		start: number;
		end: number;
	};

	const editor = getEditor();
	const FILEPATH_REGEX = /@([^ ]*)$/i;

	function getFileMatch(text: string): FileMatch | null {
		const match = FILEPATH_REGEX.exec(text);
		if (match !== null) {
			return {
				matchText: match[0],
				captureText: match[1],
				start: match.index + 1,
				end: match.index + match[0].length
			};
		}
		return null;
	}

	let fileMatch = $state<string | null>(null);

	function exit() {
		fileMatch = null;
		onExitSuggestion();
	}

	function onMatch(match: FileMatch) {
		fileMatch = match.captureText;
		onUpdateSuggestion({ loading: true }, fileMatch ?? '');
		getFileItems(fileMatch).then((items) => {
			onUpdateSuggestion({ items, loading: false }, fileMatch ?? '');
		});
	}

	export function selectFileSuggestion(path: string) {
		if (fileMatch === null) return;

		const fileStart = fileMatch;

		// Replace the search text with the selected file path
		editor.update(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;

			const anchor = selection.anchor;
			const anchorNode = anchor.getNode();
			if (!isTextNode(anchorNode)) return;

			const offset = anchor.offset;
			const fullText = anchorNode.getTextContent();
			const textBeforeCursor = fullText.slice(0, offset);
			const textAfterCursor = fullText.slice(offset);
			const match = '@' + fileStart;
			const matchIndex = textBeforeCursor.lastIndexOf(match);
			if (matchIndex !== -1) {
				const replacement = '`' + path + '`';
				const textBefore = textBeforeCursor.slice(0, matchIndex);

				// Split the text node and insert inline code node
				const beforeNode = new TextNode(textBefore);
				const inlineCodeNode = createInlineCodeNode(replacement);
				const afterNode = new TextNode(textAfterCursor);

				anchorNode.replace(beforeNode);
				beforeNode.insertAfter(inlineCodeNode);
				inlineCodeNode.insertAfter(afterNode);

				// Position cursor after the inline code
				afterNode.selectStart();
			}
		});

		exit();
	}

	export function exitFileSuggestions() {
		exit();
	}
</script>

<TypeAhead onExit={exit} testMatch={getFileMatch} {onMatch} />
