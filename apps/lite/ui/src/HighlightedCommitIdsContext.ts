import { createContext } from "react";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";

type HighlightedCommitIdsContext = {
	highlightedCommitIds: Set<string>;
	setHighlightedCommitIds: (projectId: string, commitIds: Array<string>) => void;
	clearHighlightedCommitIds: (projectId: string) => void;
};

export const HighlightedCommitIdsContext = createContext({} as HighlightedCommitIdsContext);
HighlightedCommitIdsContext.displayName = "HighlightedCommitIdsContext";

type HighlightedCommitIdsRegistryContext = ProjectRegistry<Set<string>>;

export const HighlightedCommitIdsRegistryContext = createContext(
	{} as HighlightedCommitIdsRegistryContext,
);
HighlightedCommitIdsRegistryContext.displayName = "HighlightedCommitIdsRegistryContext";
