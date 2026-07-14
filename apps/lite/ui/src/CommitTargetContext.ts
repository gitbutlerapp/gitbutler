import { createContext } from "react";
import type { BranchOperand } from "#ui/operands.ts";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";
import type { RelativeTo } from "@gitbutler/but-sdk";

type CommitTargetContext = {
	commitTarget: RelativeTo | null;
	setCommitTarget: (commitTarget: RelativeTo | null) => void;
	updateRewrittenCommitReferences: (replacedCommits: Record<string, string>) => void;
	updateRewrittenBranchReferences: (oldBranch: BranchOperand, newBranch: BranchOperand) => void;
};

export const CommitTargetContext = createContext({} as CommitTargetContext);
CommitTargetContext.displayName = "CommitTargetContext";

type CommitTargetRegistryContext = ProjectRegistry<RelativeTo | null>;

export const CommitTargetRegistryContext = createContext({} as CommitTargetRegistryContext);
CommitTargetRegistryContext.displayName = "CommitTargetRegistryContext";
