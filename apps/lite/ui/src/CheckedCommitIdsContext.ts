import { createContext } from "react";

type CheckedCommitIdsContext = {
	checkedCommitIds: Set<string>;
	setCommitsChecked: (commitIds: Array<string>, checked: boolean) => void;
	clearCheckedCommits: () => void;
};

export const CheckedCommitIdsContext = createContext({} as CheckedCommitIdsContext);
CheckedCommitIdsContext.displayName = "CheckedCommitIdsContext";

type CheckedCommitIdsRegistryContext = (projectId: string) => CheckedCommitIdsContext & {
	updateRewrittenCommitReferences: (replacedCommits: Record<string, string>) => void;
};

export const CheckedCommitIdsRegistryContext = createContext({} as CheckedCommitIdsRegistryContext);
CheckedCommitIdsRegistryContext.displayName = "CheckedCommitIdsRegistryContext";
