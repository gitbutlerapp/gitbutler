/**
 * Applying a {@link md.MarkdownCommand} to a live textarea.
 *
 * Kept apart from `markdown-editing.ts`, which stays pure and DOM-free, and
 * from any one component: both the formatting toolbar and file attachments
 * rewrite the same textarea and must do it the same way to share its undo
 * stack.
 */

import type * as md from "#ui/markdown-editing.ts";

/** The span that actually changed between two revisions of the same source. */
const changedSpan = (from: string, to: string): { start: number; end: number; insert: string } => {
	let start = 0;
	while (start < from.length && start < to.length && from[start] === to[start]) start += 1;

	let tail = 0;
	while (
		tail < from.length - start &&
		tail < to.length - start &&
		from[from.length - 1 - tail] === to[to.length - 1 - tail]
	)
		tail += 1;

	return { start, end: from.length - tail, insert: to.slice(start, to.length - tail) };
};

/**
 * Rewrite a textarea in place, returning the new source for the owner's
 * controlled state. The DOM is written first so the caret survives the round
 * trip: React skips syncing a controlled input whose node already holds the
 * value it is about to render, and only that skip preserves selection.
 *
 * The edit goes through `execCommand` over just the changed span rather than
 * an assignment to `value`, which would drop the textarea's native undo stack
 * and leave Cmd+Z undoing neither the formatting nor the typing before it.
 */
export const applyToTextarea = (
	target: HTMLTextAreaElement,
	command: md.MarkdownCommand,
): string => {
	const next = command({
		text: target.value,
		start: target.selectionStart,
		end: target.selectionEnd,
	});

	target.focus();

	const { start, end, insert } = changedSpan(target.value, next.text);
	target.setSelectionRange(start, end);
	const undoable =
		insert === ""
			? document.execCommand("delete")
			: document.execCommand("insertText", false, insert);

	// execCommand is deprecated and may refuse; the edit itself matters more
	// than its undo entry, so fall back to writing the value outright.
	if (!undoable) target.value = next.text;

	target.setSelectionRange(next.start, next.end);
	return next.text;
};
