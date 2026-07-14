import { createContext } from "react";
import type { BranchOperand } from "#ui/operands.ts";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";
import type { RelativeTo } from "@gitbutler/but-sdk";

type CommitTargetContext = {
	commitTarget: RelativeTo | null;
	setCommitTarget: (projectId: string, commitTarget: RelativeTo | null) => void;
	updateRewrittenCommitReferences: (
		projectId: string,
		replacedCommits: Record<string, string>,
	) => void;
	updateRewrittenBranchReferences: (
		projectId: string,
		oldBranch: BranchOperand,
		newBranch: BranchOperand,
	) => void;
};

export const CommitTargetContext = createContext({} as CommitTargetContext);
CommitTargetContext.displayName = "CommitTargetContext";

type CommitTargetRegistryContext = ProjectRegistry<RelativeTo | null>;

export const CommitTargetRegistryContext = createContext({} as CommitTargetRegistryContext);
CommitTargetRegistryContext.displayName = "CommitTargetRegistryContext";
