import { findBranchByRef, findCommit } from "#ui/api/ref-info.ts";
import { encodeRefName } from "#ui/api/ref-name.ts";
import { commitTitle } from "#ui/commit.ts";
import type {
	CommitGraphCommitRow,
	CommitGraphJoinRow,
	CommitGraphRow,
} from "#ui/ui/CommitGraph/commitGraphRows.ts";
import type {
	InitialBranchIntegration,
	IntegrationDivergenceCommit,
	IntegrationDivergenceDisplay,
	InteractiveIntegration,
	InteractiveIntegrationStep,
	WorkspaceState,
} from "@gitbutler/but-sdk";

const commitRow = ({
	leftRail,
	node,
	rightRail,
	commitId,
	refs,
	subject,
}: {
	leftRail: CommitGraphCommitRow["leftRail"];
	node: CommitGraphCommitRow["node"];
	rightRail: CommitGraphCommitRow["rightRail"];
	commitId: string;
	refs: Array<string>;
	subject: string;
}): CommitGraphCommitRow => ({
	kind: "commit",
	leftRail,
	node,
	rightRail,
	content: {
		commitId,
		refs,
		subject,
	},
});

const divergenceCommitRow = ({
	leftRail,
	node,
	rightRail,
	commit,
}: {
	leftRail: CommitGraphCommitRow["leftRail"];
	node: CommitGraphCommitRow["node"];
	rightRail: CommitGraphCommitRow["rightRail"];
	commit: IntegrationDivergenceCommit;
}): CommitGraphCommitRow =>
	commitRow({
		leftRail,
		node,
		rightRail,
		commitId: commit.id,
		refs: commit.refs,
		subject: commit.subject,
	});

const joinRow = (): CommitGraphJoinRow => ({
	kind: "join",
	leftRail: "|",
	node: "",
	rightRail: "/",
});

const buildCurrentStateCommitGraphRows = (
	divergence: IntegrationDivergenceDisplay,
): Array<CommitGraphRow> => {
	const rows: Array<CommitGraphRow> = [];

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
};

const formatIntegrationStep = (step: InteractiveIntegrationStep): string => {
	switch (step.kind) {
		case "pick":
		case "merge":
			return `${step.kind} ${step.commit_id}`;
		case "squash": {
			const message = step.message === null ? "" : ` | message=${JSON.stringify(step.message)}`;
			return `squash ${step.commits.join(" ")}${message}`;
		}
	}
};

const formatIntegrationScript = (integration: InteractiveIntegration): string =>
	integration.steps.map(formatIntegrationStep).join("\n");

export const integrationHints = [
	"pick <id>",
	"merge <id>",
	'squash <id> <id>... | message="..."',
].join("\n");

export const buildNextStateCommitGraphRows = ({
	workspace,
	branchRef,
}: {
	workspace: WorkspaceState;
	branchRef: string;
}): Array<CommitGraphRow> | null => {
	const encodedBranchRef = encodeRefName(branchRef);
	const branch = findBranchByRef({
		headInfo: workspace.headInfo,
		branchRef: encodedBranchRef,
	});
	if (!branch) return null;

	const { segment } = branch;
	if (segment.commits.length === 0) return [];

	const rows = segment.commits.map((commit, index) => {
		const refs = index === 0 && segment.refName ? [segment.refName.displayName] : [];
		return commitRow({
			leftRail: "",
			node: "*",
			rightRail: "",
			commitId: commit.id,
			refs,
			subject: commitTitle(commit.message),
		});
	});

	if (segment.base !== null) {
		const baseCommit = findCommit({ headInfo: workspace.headInfo, commitId: segment.base });
		rows.push(
			baseCommit
				? commitRow({
						leftRail: "",
						node: "*",
						rightRail: "",
						commitId: baseCommit.id,
						refs: [],
						subject: commitTitle(baseCommit.message),
					})
				: commitRow({
						leftRail: "",
						node: "*",
						rightRail: "",
						commitId: segment.base,
						refs: [],
						subject: "(base commit)",
					}),
		);
	}

	return rows;
};

export const seedIntegrationState = (
	initial: InitialBranchIntegration,
): { script: string; currentStateRows: Array<CommitGraphRow> } => ({
	script: formatIntegrationScript(initial.integration),
	currentStateRows: buildCurrentStateCommitGraphRows(initial.divergence),
});
