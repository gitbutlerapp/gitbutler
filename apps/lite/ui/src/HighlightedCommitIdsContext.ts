import { createContext } from "react";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";

type HighlightedCommitIdsContext = {
	highlightedCommitIds: Set<string>;
	setHighlightedCommitIds: (commitIds: Array<string>) => void;
	clearHighlightedCommitIds: () => void;
};

export const HighlightedCommitIdsContext = createContext({} as HighlightedCommitIdsContext);
HighlightedCommitIdsContext.displayName = "HighlightedCommitIdsContext";

type HighlightedCommitIdsRegistryContext = ProjectRegistry<Set<string>>;

export const HighlightedCommitIdsRegistryContext = createContext(
	{} as HighlightedCommitIdsRegistryContext,
);
HighlightedCommitIdsRegistryContext.displayName = "HighlightedCommitIdsRegistryContext";
