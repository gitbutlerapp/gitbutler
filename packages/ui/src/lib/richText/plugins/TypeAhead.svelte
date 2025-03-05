<script lang="ts" module>
	export type Match = {
		end: number;
		start: number;
	};

	type Props<MatchResult extends Match> = {
		testMatch: (text: string) => MatchResult | null;
		onExit: () => void;
		onMatch: (match: MatchResult) => void;
	};

	type T = Match;
</script>

<script lang="ts" generics="T extends Match">
	import { getEditor } from '../context';
	import {
		type EditorState,
		type RangeSelection,
		$isRangeSelection as isRangeSelection,
		$getSelection as getSelection
	} from 'lexical';

	const { onExit, testMatch, onMatch }: Props<T> = $props();

	const editor = getEditor();

	/**
	 * Get the text up to the caret position.
	 */
	function getTextUpToAnchor(selection: RangeSelection): string | null {
		const anchor = selection.anchor;
		if (anchor.type !== 'text') {
			return null;
		}
		const anchorNode = anchor.getNode();
		if (!anchorNode.isSimpleText()) {
			return null;
		}
		const anchorOffset = anchor.offset;
		return anchorNode.getTextContent().slice(0, anchorOffset);
	}

	/**
	 * Match the current editor content against the tester function.
	 */
	function matchTheCurrentSelection(editorState: EditorState) {
		editorState.read(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;

			const text = getTextUpToAnchor(selection);
			if (text === null) {
				onExit();
				return;
			}

			const match = testMatch(text);
			if (match === null) {
				onExit();
				return;
			}

			onMatch(match);
		});
	}

	// Setup the update listener
	$effect(() => {
		return editor.registerUpdateListener(
			({ editorState, dirtyElements, dirtyLeaves, prevEditorState, tags }) => {
				if (
					tags.has('history-merge') ||
					(dirtyElements.size === 0 && dirtyLeaves.size === 0) ||
					prevEditorState.isEmpty()
				) {
					// ignore irreleval updates
					return;
				}
				matchTheCurrentSelection(editorState);
			}
		);
	});
</script>
