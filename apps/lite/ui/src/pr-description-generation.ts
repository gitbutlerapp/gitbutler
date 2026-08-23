/**
 * Generating a pull request description from the branch's commits.
 *
 * Commits are the context, not the diff: they already carry the author's own
 * account of what each change was for, which is what a description is, and
 * they stay small enough to send whole where a branch diff would not.
 */

import type { Commit } from "@gitbutler/but-sdk";

export const PR_DESCRIPTION_SYSTEM_PROMPT = `You write pull requests.
Return the title on the first line, then a blank line, then the description body as Markdown.
The title is plain text: no leading "#", no quotes, no prefix.
Never repeat the title as a heading or first line of the body — it is shown above the body already.
Return nothing else: no commentary, no surrounding code fence.`;

const COMMIT_MESSAGE_LIMIT = 5_000;

/**
 * Mirrors {@link commitMessageGenerationButtonState}: the button is always
 * rendered so generation is discoverable before it is set up, and `hint`
 * replaces the plain action label in the tooltip. An unconfigured provider is
 * reported ahead of a disabled project setting, because the setting cannot be
 * turned on without one.
 */
export const prDescriptionGenerationButtonState = ({
	enabled,
	configured,
	busy,
	commitCount,
}: {
	enabled: boolean;
	configured: boolean;
	busy: boolean;
	/** `undefined` while the branch's commits are still loading. */
	commitCount: number | undefined;
}): { disabled: boolean; hint: string | null } => {
	if (!configured) return { disabled: true, hint: "Set up AI in Settings → Application → AI" };
	if (!enabled) return { disabled: true, hint: "Enable AI in Settings → Project → AI" };
	// Not yet known is not the same as none: claiming "no commits" while the
	// branch is still loading reads as a verdict rather than a wait.
	if (commitCount === undefined) return { disabled: true, hint: null };
	if (commitCount === 0) return { disabled: true, hint: "No commits to describe" };

	return { disabled: busy, hint: null };
};

/**
 * The title and body already typed are context, not something to preserve:
 * the model is asked to describe the commits, and a half-written body is a
 * hint about the intent rather than text to keep.
 */
export const buildPrDescriptionPrompt = (
	title: string,
	body: string,
	commits: ReadonlyArray<Commit>,
): string => {
	// Oldest first, so the messages read as the order the work happened in.
	const messages = commits
		.map((commit) => commit.message.trim())
		.reverse()
		.join("\n\n---\n\n")
		.slice(0, COMMIT_MESSAGE_LIMIT);

	const sections = [
		"Write the title and description for this pull request.",
		"List the most important changes. Use bullet points. Be concise.",
	];
	if (title.trim() !== "") sections.push(`Working title:\n\`\`\`\n${title.trim()}\n\`\`\``);
	if (body.trim() !== "")
		sections.push(`Description so far, to build on:\n\`\`\`\n${body.trim()}\n\`\`\``);
	sections.push(`Commit messages:\n\`\`\`\n${messages}\n\`\`\``);

	return sections.join("\n\n");
};

/**
 * Models often wrap a whole answer in a fence, and mid-stream the closing one
 * has not arrived yet — so a lone opener is dropped too, or the title would
 * briefly read "```markdown". A fence that only covers part of the answer is
 * real content and left alone.
 */
const stripSurroundingFence = (text: string): string => {
	const trimmed = text.trim();
	if (!trimmed.startsWith("```")) return trimmed;

	const fenced = /^```[a-z]*\n([\s\S]*)\n```$/.exec(trimmed);
	if (fenced?.[1] !== undefined && !fenced[1].includes("```")) return fenced[1].trim();

	const firstNewline = trimmed.indexOf("\n");
	if (firstNewline === -1) return "";
	const withoutOpener = trimmed.slice(firstNewline + 1);
	return withoutOpener.includes("```") ? trimmed : withoutOpener.trim();
};

/** Strip anything the model put around a title despite being told not to. */
const cleanTitle = (line: string): string =>
	line
		.trim()
		.replace(/^#+\s*/, "")
		.replace(/^["'`]|["'`]$/g, "")
		.trim();

/**
 * Split a generated answer into its title and body.
 *
 * The body's first line is dropped when it just restates the title: the form
 * shows the title above the body, so a heading there renders it twice. The
 * prompt asks for this too, but models do it anyway often enough to be worth
 * catching here — which is also what makes this safe to run on every partial
 * while the answer streams in.
 */
export const splitGeneratedDescription = (text: string): { title: string; body: string } => {
	const cleaned = stripSurroundingFence(text);
	const firstNewline = cleaned.indexOf("\n");
	if (firstNewline === -1) return { title: cleanTitle(cleaned), body: "" };

	const title = cleanTitle(cleaned.slice(0, firstNewline));
	const rest = cleaned.slice(firstNewline + 1).trim();

	const restNewline = rest.indexOf("\n");
	const firstBodyLine = restNewline === -1 ? rest : rest.slice(0, restNewline);
	const echoesTitle =
		title !== "" && cleanTitle(firstBodyLine).toLowerCase() === title.toLowerCase();

	return {
		title,
		body: echoesTitle ? (restNewline === -1 ? "" : rest.slice(restNewline + 1).trim()) : rest,
	};
};
