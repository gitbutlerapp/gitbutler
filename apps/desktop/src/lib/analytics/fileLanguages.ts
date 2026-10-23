import { langFromFilename } from "@gitbutler/ui-svelte/utils/shikiHighlighter";
import type { DiffSpec } from "@gitbutler/but-sdk";

// A short extension after a dot that doesn't start the file name, so `Makefile` and `.env`
// never match.
const EXTENSION = /[^/.]\.([a-z][a-z0-9]{0,9})$/i;

/**
 * Sorted, deduplicated languages of the given changes, as named by the diff highlighter.
 * Extensions it doesn't know are kept as is, which shows the languages we don't highlight
 * yet. Never returns file names.
 */
export function fileLanguages(changes: DiffSpec[]): string[] {
	const decoder = new TextDecoder();
	const languages = new Set<string>();
	for (const change of changes) {
		const path = decoder.decode(Uint8Array.from(change.pathBytes));
		const language = langFromFilename(path) ?? path.match(EXTENSION)?.[1]?.toLowerCase();
		if (language) languages.add(language);
	}
	return [...languages].sort();
}
