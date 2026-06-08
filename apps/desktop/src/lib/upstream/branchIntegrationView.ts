import type {
	Author,
	FullRefName,
	InitialBranchIntegration,
	IntegrationDivergenceCommit,
	RefInfo,
	WorkspaceState,
} from "@gitbutler/but-sdk";

export type IntegrationGraphRef = {
	name: string;
	kind: "local" | "remote" | "integrated";
};

export type IntegrationGraphRowCommit = {
	kind: "commit";
	commitKind: "local" | "remote" | "integrated";
	leftRail: string;
	node: string;
	rightRail: string;
	content: {
		commitId: string;
		refs: string[];
		refDisplays: IntegrationGraphRef[];
		subject: string;
		changeId: string | null;
		createdAt: number;
		author: Author | null;
		hasConflicts: boolean | null;
	};
};

export type IntegrationGraphRowJoin = {
	kind: "join";
	leftRail: string;
	node: string;
	rightRail: string;
};

export type IntegrationGraphRow = IntegrationGraphRowCommit | IntegrationGraphRowJoin;

function commitRow({
	leftRail,
	node,
	rightRail,
	commitKind,
	commitId,
	refs,
	refDisplays = refs.map((ref) => ({ name: ref, kind: commitKind })),
	subject,
	changeId,
	createdAt,
	author,
	hasConflicts,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commitKind: "local" | "remote" | "integrated";
	commitId: string;
	refs: string[];
	refDisplays?: IntegrationGraphRef[];
	subject: string;
	changeId: string | null;
	createdAt: number;
	author: Author | null;
	hasConflicts: boolean | null;
}): IntegrationGraphRow {
	return {
		kind: "commit",
		commitKind,
		leftRail,
		node,
		rightRail,
		content: {
			commitId,
			refs,
			refDisplays,
			subject,
			changeId,
			createdAt,
			author,
			hasConflicts,
		},
	};
}

function divergenceCommitRow({
	leftRail,
	node,
	rightRail,
	commitKind,
	commit,
	refNames,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commitKind: "local" | "remote" | "integrated";
	commit: IntegrationDivergenceCommit;
	refNames: IntegrationGraphRefNames;
}): IntegrationGraphRow {
	return commitRow({
		leftRail,
		node,
		rightRail,
		commitKind,
		commitId: commit.id,
		refs: commit.refs,
		refDisplays: commit.refs.map((ref) => ({
			name: ref,
			kind: kindForDivergenceRef(ref, refNames, commitKind),
		})),
		subject: commit.subject,
		changeId: commit.changeId,
		createdAt: commit.createdAt,
		author: commit.author,
		hasConflicts: null,
	});
}

function joinRow(): IntegrationGraphRow {
	return {
		kind: "join",
		leftRail: "|",
		node: "",
		rightRail: "/",
	};
}

function decodeRefName(fullNameBytes: number[] | null | undefined): string | null {
	if (!fullNameBytes) return null;
	return new TextDecoder().decode(Uint8Array.from(fullNameBytes));
}

function commitTitle(message: string): string {
	return message.split("\n")[0] ?? message;
}

type IntegrationGraphRefNames = {
	branch: string;
	upstream: string;
};

function shortenRefName(refName: FullRefName): string {
	const fullName = refName.full;
	if (fullName.startsWith("refs/heads/")) return fullName.slice("refs/heads/".length);
	if (fullName.startsWith("refs/remotes/")) return fullName.slice("refs/remotes/".length);
	return fullName;
}

function kindForDivergenceRef(
	ref: string,
	refNames: IntegrationGraphRefNames,
	fallback: IntegrationGraphRef["kind"],
): IntegrationGraphRef["kind"] {
	if (ref === refNames.branch) return "local";
	if (ref === refNames.upstream) return "remote";
	return fallback;
}

function findSegmentByBranchRef({ headInfo, branchRef }: { headInfo: RefInfo; branchRef: string }) {
	for (const stack of headInfo.stacks) {
		for (const segment of stack.segments) {
			if (decodeRefName(segment.refName?.fullNameBytes) === branchRef) return segment;
		}
	}
	return null;
}

function findCommit({ headInfo, commitId }: { headInfo: RefInfo; commitId: string }) {
	for (const stack of headInfo.stacks) {
		for (const segment of stack.segments) {
			const commit = segment.commits.find((candidate) => candidate.id === commitId);
			if (commit) return commit;
		}
	}
	return null;
}

export function buildCurrentStateGraphRows(
	initialIntegration: InitialBranchIntegration,
): IntegrationGraphRow[] {
	const { divergence } = initialIntegration;
	const rows: IntegrationGraphRow[] = [];
	const refNames = {
		branch: shortenRefName(divergence.branchRefName),
		upstream: shortenRefName(divergence.upstreamRefName),
	};

	for (const commit of divergence.localOnly) {
		const commitKind = commit.targetRelation.kind === "notIntegrated" ? "local" : "integrated";
		rows.push(
			divergenceCommitRow({
				leftRail: "",
				node: "*",
				rightRail: "",
				commitKind,
				commit,
				refNames,
			}),
		);
	}

	for (const commit of divergence.upstreamOnly) {
		const commitKind = commit.targetRelation.kind === "notIntegrated" ? "remote" : "integrated";
		rows.push(
			divergenceCommitRow({
				leftRail: divergence.localOnly.length === 0 ? "" : "|",
				node: "*",
				rightRail: "",
				commitKind,
				commit,
				refNames,
			}),
		);
	}

	if (divergence.localOnly.length > 0 && divergence.upstreamOnly.length > 0) rows.push(joinRow());

	const commitKind =
		divergence.mergeBase.targetRelation.kind === "notIntegrated" ? "remote" : "integrated";
	rows.push(
		divergenceCommitRow({
			leftRail: "",
			node: "*",
			rightRail: "",
			commitKind,
			commit: divergence.mergeBase,
			refNames,
		}),
	);

	return rows;
}

export function buildNextStateGraphRows({
	workspace,
	branchRef,
}: {
	workspace: WorkspaceState;
	branchRef: string;
}): IntegrationGraphRow[] | null {
	const segment = findSegmentByBranchRef({
		headInfo: workspace.headInfo,
		branchRef,
	});
	if (!segment) return null;
	if (segment.commits.length === 0) return [];

	const now = new Date();

	const rows = segment.commits.map((commit, index) =>
		commitRow({
			leftRail: "",
			node: "*",
			rightRail: "",
			commitKind: "local",
			commitId: commit.id,
			refs: index === 0 && segment.refName ? [segment.refName.displayName] : [],
			subject: commitTitle(commit.message),
			changeId: commit.changeId,
			createdAt: now.getTime(),
			author: commit.author,
			hasConflicts: commit.hasConflicts,
		}),
	);

	if (segment.base !== null) {
		const baseCommit = findCommit({
			headInfo: workspace.headInfo,
			commitId: segment.base,
		});
		rows.push(
			commitRow({
				leftRail: "",
				node: "*",
				rightRail: "",
				commitKind: "integrated",
				commitId: segment.base,
				refs: [],
				subject: baseCommit ? commitTitle(baseCommit.message) : "(base commit)",
				changeId: null,
				createdAt: baseCommit ? Number(baseCommit.createdAt) : 0,
				author: baseCommit?.author ?? null,
				hasConflicts: baseCommit?.hasConflicts ?? null,
			}),
		);
	}

	return rows;
}
