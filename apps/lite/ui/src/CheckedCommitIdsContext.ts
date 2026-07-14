import { createContext } from "react";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";

type CheckedCommitIdsContext = {
	checkedCommitIds: Set<string>;
	setCommitsChecked: (projectId: string, commitIds: Array<string>, checked: boolean) => void;
	clearCheckedCommits: (projectId: string) => void;
	updateRewrittenCommitReferences: (
		projectId: string,
		replacedCommits: Record<string, string>,
	) => void;
};

export const CheckedCommitIdsContext = createContext({} as CheckedCommitIdsContext);
CheckedCommitIdsContext.displayName = "CheckedCommitIdsContext";

type CheckedCommitIdsRegistryContext = ProjectRegistry<Set<string>>;

export const CheckedCommitIdsRegistryContext = createContext({} as CheckedCommitIdsRegistryContext);
CheckedCommitIdsRegistryContext.displayName = "CheckedCommitIdsRegistryContext";
