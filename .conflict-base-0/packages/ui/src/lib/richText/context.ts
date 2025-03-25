import { getContext } from 'svelte';
import type { LexicalEditor } from 'lexical';

export function getEditor(): LexicalEditor {
	return getContext('editor');
}
