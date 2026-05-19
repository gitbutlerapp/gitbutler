export type CommitGraphRail = "" | "|";
export type CommitGraphRightRail = "" | "|" | "/";

export type CommitGraphCommitRow = {
	kind: "commit";
	leftRail: CommitGraphRail;
	node: "*";
	rightRail: CommitGraphRightRail;
	content: {
		commitId: string;
		refs: Array<string>;
		subject: string;
	};
};

export type CommitGraphJoinRow = {
	kind: "join";
	leftRail: CommitGraphRail;
	node: "";
	rightRail: "/";
};

export type CommitGraphRow = CommitGraphCommitRow | CommitGraphJoinRow;
