import {
	getSwallowGitHubOrgAuthErrors,
	persistSwallowGitHubOrgAuthErrors,
} from "$lib/config/config";
import { SilentError } from "$lib/error/error";
import { parseError } from "$lib/error/parser";
import type { Code } from "@gitbutler/but-sdk";

export type Severity = "error" | "warning" | "silent";

export type ActionHint = {
	label: string;
	onClick: () => void;
};

/**
 * UX classification for a backend error. Keyed by `Code` in the
 * `CLASSIFICATIONS` table below.
 *
 * `severity` drives the toast style and which capture event fires:
 * `error` → danger style + Sentry capture; `warning` → warning style,
 * no Sentry; `silent` → suppress the toast and capture entirely.
 *
 * `userMessage` is the long-form, user-facing description rendered in
 * the toast body. `actionHint` adds an optional CTA button.
 */
export type Classification = {
	severity: Severity;
	userMessage?: string;
	actionHint?: ActionHint;
};

export type ClassifiedError = {
	title: string;
	message: string;
	code?: Code;
	severity: Severity;
	userMessage?: string;
	actionHint?: ActionHint;
};

const GH_ORG_AUTH_ERROR = "GitHub Organizations OAuth Error";

/**
 * Rewrite distinctive raw-message prefixes to a stable title so two
 * variants of the same root cause land under the same Sentry/PostHog
 * bucket — and so title-keyed rules below can match.
 */
const MESSAGE_PATTERN_TITLES: Record<string, string> = {
	"Although you appear to have the correct authorization credentials,": GH_ORG_AUTH_ERROR,
};

const GITHUB_ORG_AUTH_ACTION: ActionHint = {
	label: "Don't show this again",
	onClick: () => persistSwallowGitHubOrgAuthErrors(true),
};

/**
 * Per-`Code` presentation rules. This table is the single source of
 * truth for "how should a backend error code show up to users?" — add
 * a code-keyed entry here rather than special-casing inside callers
 * or `showError`.
 *
 * `userMessage` long-form text used to live in `knownErrors.ts`; it's
 * folded in here so severity, copy, and action live together.
 */
const CLASSIFICATIONS: Partial<Record<Code, Classification>> = {
	PreconditionFailed: {
		severity: "warning",
	},
	/**
	 * Transport-level failure reaching a forge (DNS, timeout, connection
	 * refused). The user is effectively offline; surfacing a toast every
	 * time a polled `list_reviews` round-trip fails is just noise.
	 */
	NetworkError: {
		severity: "silent",
	},
	/**
	 * Auto-fetch / fetch-from-remotes failure caused by missing
	 * credentials. Soft style — the user can fix it from project settings.
	 */
	ProjectGitAuth: {
		severity: "warning",
		userMessage: "Authentication failed. Check that your git credentials are configured correctly.",
	},
	/**
	 * Surfaced when there's no default target — the workspace router
	 * sends the user to the project-setup page, so a toast on top of
	 * that would be redundant noise.
	 */
	DefaultTargetNotFound: {
		severity: "silent",
	},
	CommitSigningFailed: {
		severity: "error",
		userMessage: `
Commit signing failed and has now been disabled. You can configure commit signing in the project settings.

Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/signing-commits) on setting up commit signing and verification.
		`,
	},
	RepoOwnership: {
		severity: "error",
		userMessage: `
The repository ownership couldn't be determined. Consider allowing it using:

    git config --global --add safe.directory copy/of/path/shown/below
	`,
	},
	SecretKeychainNotFound: {
		severity: "error",
		userMessage: `
Please install a keychain service to store and retrieve secrets with.

This can be done using \`sudo apt install gnome-keyring\` for instance.
	`,
	},
	MissingLoginKeychain: {
		severity: "error",
		userMessage: `
Missing default keychain.

With \`seahorse\` or equivalent, create a \`Login\` password store, right click it and choose \`Set Default\`.
	`,
	},
	GitHubTokenExpired: {
		severity: "error",
		userMessage: `
Your GitHub token appears expired. Please log out and back in to refresh it. (Settings -> Integrations -> Forget)
	`,
	},
	ProjectDatabaseIncompatible: {
		severity: "error",
		userMessage: `
The database was changed by a more recent version of GitButler - cannot safely open it anymore.
	`,
	},
	DefaultTerminalNotFound: {
		severity: "error",
		userMessage: `
Your default terminal was not found. Please select your preferred terminal in Settings > General.
	`,
	},
};

/**
 * Vite occasionally produces this unrecoverable bundling failure; the
 * resolution is a manual cache-disable and reload. Surfacing it as a
 * toast or capturing it just produces noise.
 */
function isUnrecoverableBundlingError(message: string): boolean {
	return message.startsWith("undefined is not an object (evaluating 'first_child_getter.call')");
}

/**
 * Message-pattern rules for cases where `Code` alone isn't specific
 * enough — e.g. a generic `Unknown` code whose `message` body
 * identifies a known dev-environment problem. Checked before the
 * code-keyed table, so a pattern match wins when both apply.
 *
 * Keep this list short; reach for a real `Code` first if at all
 * possible.
 */
const MESSAGE_PATTERNS: ReadonlyArray<{
	matches: (parsed: { code?: Code; message: string }) => boolean;
	classification: Classification;
}> = [
	{
		matches: ({ code, message }) =>
			code === "Unknown" && message.includes("cargo build -p gitbutler-git"),
		classification: {
			severity: "error",
			userMessage:
				"The `gitbutler-git` binary is missing. Run `cargo build -p gitbutler-git` to build it.",
		},
	},
];

function titleFromMessagePattern(message: string): string | undefined {
	for (const [prefix, title] of Object.entries(MESSAGE_PATTERN_TITLES)) {
		if (message.startsWith(prefix)) return title;
	}
	return undefined;
}

/**
 * Combine the parsed error with the per-code classification table
 * and title/message heuristics into a single presentation decision.
 *
 * Returns `severity: 'silent'` for anything that should not surface
 * (bundling noise, `SilentError`, the parser's "Load failed" ignore,
 * or a previously-opted-out GitHub-org-auth error).
 */
export function classify(error: unknown, callerTitle?: string): ClassifiedError {
	if (error instanceof SilentError) {
		return {
			title: callerTitle ?? error.name,
			message: error.message,
			severity: "silent",
		};
	}

	const { name, message, code, origin } = parseError(error);
	const title = name ?? titleFromMessagePattern(message) ?? callerTitle ?? message;

	if (isUnrecoverableBundlingError(message)) {
		return { title, message, code, severity: "silent" };
	}
	// Silence octokit's offline "Load failed" — happens whenever the user
	// loses network, surfaces nothing actionable.
	if (origin === "http" && message === "Load failed") {
		return { title, message, code, severity: "silent" };
	}
	if (title === GH_ORG_AUTH_ERROR && getSwallowGitHubOrgAuthErrors()) {
		return { title, message, code, severity: "silent" };
	}

	const byMessage = MESSAGE_PATTERNS.find((p) => p.matches({ code, message }))?.classification;
	const byCode = code ? CLASSIFICATIONS[code] : undefined;
	const effective = byMessage ?? byCode;
	if (effective?.severity === "silent") {
		return { title, message, code, severity: "silent" };
	}

	const actionHint =
		effective?.actionHint ?? (title === GH_ORG_AUTH_ERROR ? GITHUB_ORG_AUTH_ACTION : undefined);

	return {
		title,
		message,
		code,
		severity: effective?.severity ?? "error",
		userMessage: effective?.userMessage,
		actionHint,
	};
}
