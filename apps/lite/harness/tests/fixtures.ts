import type {
	AiConfiguration,
	Commit,
	ProjectForFrontend,
	RefInfo,
	Segment,
	Stack,
	TreeChange,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import type { GUISettings } from "#electron/settings.ts";
import type { FakeHandlers } from "./fake-transport.ts";

/**
 * Hand-built backend payloads, kept honest by their types: a schema change
 * breaks the rig at compile time instead of letting it drift from the app.
 */

const encode = (text: string): Array<number> => Array.from(new TextEncoder().encode(text));

// Fixed dates: relative-time labels must not depend on when the test runs.
const AUTHORED_AT = Date.UTC(2026, 0, 1);

export const fixtureCommit = ({ id, message }: { id: string; message: string }): Commit => ({
	id,
	parentIds: [],
	message,
	hasConflicts: false,
	state: { type: "LocalOnly" },
	authoredAt: AUTHORED_AT,
	committedAt: AUTHORED_AT,
	author: { name: "Fixture", email: "fixture@example.com", gravatarUrl: "" },
	changeId: `change-${id}`,
	gerritReviewUrl: null,
});

export const fixtureSegment = ({
	branch,
	commits,
}: {
	branch: string;
	commits: Array<Commit>;
}): Segment => ({
	refName: { fullNameBytes: encode(`refs/heads/${branch}`), displayName: branch },
	remoteTrackingRefName: null,
	commits,
	commitsOnRemote: [],
	commitsOutside: null,
	metadata: null,
	isEntrypoint: false,
	pushStatus: "completelyUnpushed",
	base: "0".repeat(40),
});

// Real workspaces give every stack an id; lists key rows on it.
const fixtureStack = (segments: Array<Segment>, index: number): Stack => ({
	id: `stack-${index}`,
	base: "0".repeat(40),
	segments,
});

export const fixtureHeadInfo = (stacks: Array<Array<Segment>>): RefInfo => ({
	workspaceRef: {
		fullNameBytes: encode("refs/heads/gitbutler/workspace"),
		displayName: "gitbutler/workspace",
	},
	stacks: stacks.map(fixtureStack),
	target: null,
	isManagedRef: true,
	isManagedCommit: true,
	isEntrypoint: true,
	worktrees: [],
});

export const fixtureFileChange = (path: string): TreeChange => ({
	path,
	pathBytes: encode(path),
	status: {
		type: "Modification",
		subject: {
			previousState: { id: "1".repeat(40), kind: "Blob" },
			state: { id: "2".repeat(40), kind: "Blob" },
			flags: null,
		},
	},
});

export const fixtureWorktreeChanges = (changes: Array<TreeChange>): WorktreeChanges => ({
	changes,
	ignoredChanges: [],
	assignments: [],
	assignmentsError: null,
	dependencies: null,
	dependenciesError: null,
});

const fixtureProject = (id: string): ProjectForFrontend => ({
	id,
	title: "Fixture Project",
	description: null,
	path: `/fixtures/${id}`,
	api: null,
	omit_certificate_check: null,
	snapshot_lines_threshold: null,
	forge_override: null,
	preferred_forge_user: null,
	forge_review_template_path: null,
	is_open: true,
});

const fixtureAiConfiguration: AiConfiguration = {
	provider: "anthropic",
	openaiKeyOption: "butlerAPI",
	openaiModel: "",
	openaiHasApiKey: false,
	anthropicKeyOption: "butlerAPI",
	anthropicModel: "",
	anthropicHasApiKey: false,
	ollamaEndpoint: "",
	ollamaModel: "",
	lmstudioEndpoint: "",
	lmstudioModel: "",
	isConfigured: false,
};

/**
 * The app-wide queries every mounted panel runs regardless of fixture: enough
 * of an answer for the components to render, nothing project-specific.
 */
export const globalHandlers = (projectId: string): FakeHandlers => ({
	readGUISettings: (): GUISettings => ({ version: 1 }),
	listEditors: () => [],
	getAiConfiguration: () => fixtureAiConfiguration,
	forgeInfo: () => null,
	listProjectsStateless: () => [fixtureProject(projectId)],
	// "No patch": good enough until a test renders a diff.
	treeChangeDiffs: () => null,
	branchCannedName: () => "canned-branch-name",
});
