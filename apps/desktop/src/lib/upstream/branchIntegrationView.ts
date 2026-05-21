import type {
	InitialBranchIntegration,
	IntegrationDivergenceCommit,
	RefInfo,
	WorkspaceState,
} from "@gitbutler/but-sdk";

export type IntegrationGraphRow =
	| {
			kind: "commit";
			leftRail: string;
			node: string;
			rightRail: string;
			content: {
				commitId: string;
				refs: string[];
				subject: string;
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
	commitId,
	refs,
	subject,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commitId: string;
	refs: string[];
	subject: string;
}): IntegrationGraphRow {
	return {
		kind: "commit",
		leftRail,
		node,
		rightRail,
		content: {
			commitId,
			refs,
			subject,
		},
	};
}

function divergenceCommitRow({
	leftRail,
	node,
	rightRail,
	commit,
}: {
	leftRail: string;
	node: string;
	rightRail: string;
	commit: IntegrationDivergenceCommit;
}): IntegrationGraphRow {
	return commitRow({
		leftRail,
		node,
		rightRail,
		commitId: commit.id,
		refs: commit.refs,
		subject: commit.subject,
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

	for (const commit of divergence.localOnly)
		rows.push(
			divergenceCommitRow({
				leftRail: "",
				node: "*",
				rightRail: "",
				commit,
			}),
		);

	for (const commit of divergence.upstreamOnly)
		rows.push(
			divergenceCommitRow({
				leftRail: divergence.localOnly.length === 0 ? "" : "|",
				node: "*",
				rightRail: "",
				commit,
			}),
		);

	if (divergence.localOnly.length > 0 && divergence.upstreamOnly.length > 0) rows.push(joinRow());

	for (const commit of divergence.matched)
		rows.push(
			divergenceCommitRow({
				leftRail: "",
				node: "*",
				rightRail: "",
				commit,
			}),
		);

	rows.push(
		divergenceCommitRow({
			leftRail: "",
			node: "*",
			rightRail: "",
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

	const rows = segment.commits.map((commit, index) =>
		commitRow({
			leftRail: "",
			node: "*",
			rightRail: "",
			commitId: commit.id,
			refs: index === 0 && segment.refName ? [segment.refName.displayName] : [],
			subject: commitTitle(commit.message),
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
				commitId: segment.base,
				refs: [],
				subject: baseCommit ? commitTitle(baseCommit.message) : "(base commit)",
			}),
		);
	}

	return rows;
}
