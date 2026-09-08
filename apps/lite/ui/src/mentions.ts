/** `@mention` completion for a plain textarea. Pure and DOM-free, like `markdown-editing.ts`. */

import type { MarkdownCommand, MarkdownSelection } from "#ui/markdown-editing.ts";
import type { ForgeReviewUser } from "@gitbutler/but-sdk";

/** Where a mention's `@` sits and what has been typed after it. */
type MentionDraft = { readonly at: number; readonly query: string };

const isLoginChar = (char: string): boolean => /[\w-]/.test(char);

/**
 * The mention being typed at the caret, if any: a collapsed caret at the end
 * of an `@word` whose `@` starts the word, so an email address does not count.
 */
export const mentionAtCaret = ({ text, start, end }: MarkdownSelection): MentionDraft | null => {
	if (start !== end || isLoginChar(text.charAt(end))) return null;
	let at = end;
	while (at > 0 && isLoginChar(text.charAt(at - 1))) at -= 1;
	if (text.charAt(at - 1) !== "@" || isLoginChar(text.charAt(at - 2))) return null;
	return { at: at - 1, query: text.slice(at, end) };
};

const LIMIT = 8;

/** Who the query could mean: logins starting with it first, then anyone whose login or name contains it. */
export const matchMentions = (
	candidates: ReadonlyArray<ForgeReviewUser>,
	query: string,
): Array<ForgeReviewUser> => {
	const needle = query.toLowerCase();
	const contains = (text: string | null) => text?.toLowerCase().includes(needle) ?? false;
	const leads = (user: ForgeReviewUser) => user.login.toLowerCase().startsWith(needle);
	return candidates
		.filter((user) => contains(user.login) || contains(user.name))
		.sort((a, b) => Number(leads(b)) - Number(leads(a)))
		.slice(0, LIMIT);
};

/**
 * Replace the mention at `at`, up to the caret, with `@login` and leave the
 * caret after a following space, reusing one already there.
 */
export const completeMention =
	(at: number, login: string): MarkdownCommand =>
	({ text, end }) => {
		const mention = `@${login}`;
		const tail = text.slice(end);
		const after = at + mention.length + 1;
		return {
			text: `${text.slice(0, at)}${mention}${tail.startsWith(" ") ? "" : " "}${tail}`,
			start: after,
			end: after,
		};
	};
