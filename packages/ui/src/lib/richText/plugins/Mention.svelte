<script lang="ts" module>
	export type MentionSuggestion = {
		id: string;
		label: string;
	};

	export type MentionSuggestionUpdate = {
		items: MentionSuggestion[];
	};
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getMentionMatch, insertMention, type MentionMatch } from '$lib/richText/node/mention';
	import TypeAhead from '$lib/richText/plugins/TypeAhead.svelte';
	import { $getSelection as getSelection } from 'lexical';

	type Props = {
		getSuggestionItems: (q: string) => Promise<MentionSuggestion[]>;
		onUpdateSuggestion: (p: MentionSuggestionUpdate) => void;
		onExitSuggestion: () => void;
	};

	const { getSuggestionItems, onUpdateSuggestion, onExitSuggestion }: Props = $props();

	const editor = getEditor();

	let mentionMatch = $state<MentionMatch | null>(null);

	function exit() {
		mentionMatch = null;
		onExitSuggestion();
	}

	function onMatch(match: MentionMatch) {
		mentionMatch = match;
		getSuggestionItems(match.username).then((items) => {
			onUpdateSuggestion({ items });
		});
	}

	export function selectMentionSuggestion(suggestion: MentionSuggestion) {
		if (mentionMatch === null) return;

		const mentionStart = mentionMatch.start;
		const mentionEnd = mentionMatch.end;

		// Replace the search text with the selected mention
		editor.update(
			() => {
				const selection = getSelection();
				insertMention({
					selection,
					start: mentionStart,
					end: mentionEnd,
					id: suggestion.id,
					label: suggestion.label
				});
			},
			{ tag: 'history-merge' }
		);

		exit();
	}

	export function exitSuggestions() {
		exit();
	}
</script>

<TypeAhead onExit={exit} testMatch={getMentionMatch} {onMatch} />
