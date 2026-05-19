export type GitTermKey =
	| "amend"
	| "branch"
	| "cherry-pick"
	| "commit"
	| "fetch"
	| "merge"
	| "merge-conflict"
	| "pull"
	| "push"
	| "rebase"
	| "reset"
	| "squash"
	| "stash"
	| "upstream";

type GitGlossaryEntry = {
	aliases: readonly string[];
	explanation: string;
};

export const gitGlossary = {
	amend: {
		aliases: ["amend", "amends", "amended"],
		explanation: "Replaces your most recent commit with an updated version.",
	},
	branch: {
		aliases: ["branch", "branches"],
		explanation: "A named line of work so you can make changes separately from other work.",
	},
	"cherry-pick": {
		aliases: ["cherry-pick", "cherry-picks", "cherry-picked"],
		explanation: "Copies one specific commit onto another branch.",
	},
	commit: {
		aliases: ["commit", "commits"],
		explanation: "A saved snapshot of your changes.",
	},
	fetch: {
		aliases: ["fetch", "fetches", "fetched"],
		explanation: "Downloads remote changes without applying them to your current branch.",
	},
	merge: {
		aliases: ["merge", "merges", "merged"],
		explanation: "Combines changes from two lines of work into one.",
	},
	"merge-conflict": {
		aliases: [
			"merge conflict",
			"merge conflicts",
			"conflict",
			"conflicts",
			"conflict marker",
			"conflict markers",
		],
		explanation:
			"A place where Git cannot combine changes automatically and needs you to choose what to keep.",
	},
	pull: {
		aliases: ["pull", "pulls", "pulled"],
		explanation: "Downloads remote changes and applies them to your current branch.",
	},
	push: {
		aliases: ["push", "pushes", "pushed"],
		explanation: "Sends your local commits to the remote repository.",
	},
	rebase: {
		aliases: ["rebase", "rebases", "rebased"],
		explanation: "Moves your commits onto a new base so they sit on top of newer changes.",
	},
	reset: {
		aliases: ["reset", "resets", "hard reset"],
		explanation:
			"Moves your branch pointer to another commit and can optionally throw away local changes.",
	},
	squash: {
		aliases: ["squash", "squashes", "squashed"],
		explanation: "Combines several commits into one commit.",
	},
	stash: {
		aliases: ["stash", "stashes", "stashed"],
		explanation:
			"Temporarily saves uncommitted changes so you can switch context without losing them.",
	},
	upstream: {
		aliases: ["upstream"],
		explanation: "The remote branch your current work is based on or compared against.",
	},
} satisfies Record<GitTermKey, GitGlossaryEntry>;

export const allGitTermKeys = Object.keys(gitGlossary) as GitTermKey[];

export type GlossarySegment =
	| { type: "text"; text: string }
	| { type: "term"; text: string; term: GitTermKey };

type GlossaryMatcher = {
	regex: RegExp;
	aliasToTerm: Map<string, GitTermKey>;
};

function escapeRegex(text: string): string {
	return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function normalizeGlossaryAlias(text: string): string {
	return text.trim().toLowerCase();
}

function buildGlossaryMatcher(terms: readonly GitTermKey[]): GlossaryMatcher | undefined {
	const uniqueTerms = [...new Set(terms)];
	const aliasToTerm = new Map<string, GitTermKey>();
	const aliases = uniqueTerms.flatMap((term) =>
		gitGlossary[term].aliases.map((alias) => {
			aliasToTerm.set(normalizeGlossaryAlias(alias), term);
			return alias;
		}),
	);

	if (aliases.length === 0) {
		return undefined;
	}

	const pattern = aliases
		.sort((left, right) => right.length - left.length)
		.map((alias) => escapeRegex(alias))
		.join("|");

	return {
		regex: new RegExp(`(?<![A-Za-z0-9])(${pattern})(?![A-Za-z0-9])`, "gi"),
		aliasToTerm,
	};
}

export function getGitGlossaryExplanation(term: GitTermKey): string {
	return gitGlossary[term].explanation;
}

export function splitGlossaryText(
	text: string,
	terms: readonly GitTermKey[] = allGitTermKeys,
): GlossarySegment[] {
	if (text.length === 0) {
		return [];
	}

	const matcher = buildGlossaryMatcher(terms);
	if (!matcher) {
		return [{ type: "text", text }];
	}

	const segments: GlossarySegment[] = [];
	let lastIndex = 0;

	for (const match of text.matchAll(matcher.regex)) {
		const index = match.index ?? 0;
		const matchedText = match[0];
		const normalizedAlias = normalizeGlossaryAlias(matchedText);
		const term = matcher.aliasToTerm.get(normalizedAlias);

		if (!term) {
			continue;
		}

		if (index > lastIndex) {
			segments.push({ type: "text", text: text.slice(lastIndex, index) });
		}

		segments.push({ type: "term", text: matchedText, term });
		lastIndex = index + matchedText.length;
	}

	if (segments.length === 0) {
		return [{ type: "text", text }];
	}

	if (lastIndex < text.length) {
		segments.push({ type: "text", text: text.slice(lastIndex) });
	}

	return segments;
}
