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
	import { getEditor } from '$lib/richText/context';
	import { getLineTextUpToAnchor } from '$lib/richText/selection';
	import {
		type EditorState,
		$isRangeSelection as isRangeSelection,
		$getSelection as getSelection
	} from 'lexical';

	const { onExit, testMatch, onMatch }: Props<T> = $props();

	const editor = getEditor();

	/**
	 * Match the current editor content against the tester function.
	 */
	function matchTheCurrentSelection(editorState: EditorState) {
		editorState.read(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;

			const text = getLineTextUpToAnchor(selection);
			if (text === undefined) {
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
