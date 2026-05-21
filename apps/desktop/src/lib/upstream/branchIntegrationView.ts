import type {
	Author,
	InitialBranchIntegration,
	IntegrationDivergenceCommit,
	RefInfo,
	WorkspaceState,
} from "@gitbutler/but-sdk";

export type IntegrationGraphRow =
	| {
			kind: "commit";
			commitKind: "local" | "remote" | "integrated";
			leftRail: string;
			node: string;
			rightRail: string;
			content: {
				commitId: string;
				refs: string[];
				subject: string;
				changeId: string | null;
				createdAt: number;
				author: Author | null;
			};
	  }
	| {
			kind: "join";
			leftRail: string;
			node: string;
			rightRail: string;
	  };

function commitRow({
	leftRail,
	node,
	rightRail,
	commitKind,
	commitId,
	refs,
	subject,
	changeId,
	createdAt,
	author,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commitKind: "local" | "remote" | "integrated";
	commitId: string;
	refs: string[];
	subject: string;
	changeId: string | null;
	createdAt: number;
	author: Author | null;
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
			subject,
			changeId,
			createdAt,
			author,
		},
	};
}

function divergenceCommitRow({
	leftRail,
	node,
	rightRail,
	commitKind,
	commit,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commitKind: "local" | "remote" | "integrated";
	commit: IntegrationDivergenceCommit;
}): IntegrationGraphRow {
	return commitRow({
		leftRail,
		node,
		rightRail,
		commitKind,
		commitId: commit.id,
		refs: commit.refs,
		subject: commit.subject,
		changeId: commit.changeId,
		createdAt: commit.createdAt,
		author: commit.author,
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

	for (const commit of divergence.localOnly) {
		const commitKind = commit.targetRelation.kind === "notIntegrated" ? "local" : "integrated";
		rows.push(
			divergenceCommitRow({
				leftRail: "",
				node: "*",
				rightRail: "",
				commitKind,
				commit,
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
			}),
		);
	}

	return rows;
}
