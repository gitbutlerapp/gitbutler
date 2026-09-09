import type { ForgeInfo, ForgeUser } from "@gitbutler/but-sdk";
import type { CacheTag } from "@gitbutler/but-sdk/cache-tags";

/** Cached forge data that depends on the configured accounts and their access. */
export const forgeAuthTags: ReadonlyArray<CacheTag> = [
	"ForgeAccounts",
	"ForgeInfo",
	"ForgeLogin",
	"RepoInfo",
	"Reviews",
	"ReviewComments",
	"ReviewThreads",
	"ReviewTimeline",
	"ReviewSubmissions",
	"ReviewReactions",
	"CommentReactions",
	"MergeStatus",
	"Checks",
	"RepoLabels",
	"ReviewerCandidates",
];

export type ForgeDestination = { name: ForgeUser["provider"]; label: string; host: string };

export const forgeDestination = (
	info: ForgeInfo | null | undefined,
	reviewUrl?: string,
): ForgeDestination | null => {
	const url = reviewUrl ?? info?.baseUrl;
	if (url === undefined) return null;

	const name = info?.name;

	let label: string;
	switch (name) {
		case "github":
			label = "GitHub";
			break;
		case "gitlab":
			label = "GitLab";
			break;
		case "bitbucket":
			label = "Bitbucket";
			break;
		default:
			return null;
	}

	return { name, label, host: new URL(url).host };
};

export const isCloudForge = (destination: ForgeDestination): boolean => {
	let cloudHost: string;
	switch (destination.name) {
		case "github":
			cloudHost = "github.com";
			break;
		case "gitlab":
			cloudHost = "gitlab.com";
			break;
		case "bitbucket":
			cloudHost = "bitbucket.org";
			break;
	}

	return cloudHost === destination.host;
};

// https://linear.app/gitbutler/issue/GB-1479/use-error-codes-to-improve-displayhandling-of-backend-errors
export const forgeAuthFailure = (error: unknown): "missing" | "rejected" | null => {
	const msg = error instanceof Error ? error.message : typeof error === "string" ? error : null;
	if (msg === null) return null;

	if (msg.startsWith("Not authenticated with")) return "missing";
	if (msg.endsWith("authentication failed.")) return "rejected";

	return null;
};
