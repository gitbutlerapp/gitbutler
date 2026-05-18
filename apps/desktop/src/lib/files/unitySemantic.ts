import type { TreeChange } from "@gitbutler/but-sdk";
import type { LineId } from "@gitbutler/ui/utils/diffParsing";

export type UnityChangeKind = "added" | "removed" | "modified" | "moved" | "unchanged";
export type UnityFileKind = "scene" | "prefab";
export type UnityNodeKind = "gameObject" | "component" | "property" | "prefabOverride" | "warning";
export type UnitySelectionMode = "precise" | "hunk" | "file" | "unavailable";

export type UnitySelectableHunk = {
	oldStart: number;
	oldLines: number;
	newStart: number;
	newLines: number;
	lines: LineId[];
};

export type UnitySelection = {
	mode: UnitySelectionMode;
	hunks: UnitySelectableHunk[];
};

export type UnitySemanticChange = {
	label: string;
	propertyPath: string;
	oldValue?: string;
	newValue?: string;
	changeKind: UnityChangeKind;
	selection: UnitySelection;
};

export type UnitySemanticNode = {
	id: string;
	label: string;
	kind: UnityNodeKind;
	changeKind: UnityChangeKind;
	path: string;
	className?: string;
	children: UnitySemanticNode[];
	changes: UnitySemanticChange[];
	selection: UnitySelection;
};

export type UnitySemanticDiff = {
	fileKind: UnityFileKind;
	summary: {
		objectsChanged: number;
		componentsChanged: number;
		prefabOverridesChanged: number;
		propertiesChanged: number;
		warnings: number;
	};
	nodes: UnitySemanticNode[];
	warnings: { message: string; line?: number }[];
	rawAvailable: boolean;
};

export type UnitySmartMergeStatus = {
	available: boolean;
	command?: string;
	message: string;
};

export type UnitySmartMergeOutcome = {
	success: boolean;
	unresolvedMarkersRemaining: boolean;
	fileChanged: boolean;
	stdout: string;
	stderr: string;
	message: string;
};

export function isUnitySceneOrPrefabPath(path: string): boolean {
	const lowerPath = path.toLowerCase();
	return lowerPath.endsWith(".unity") || lowerPath.endsWith(".prefab");
}

export function unityFileKind(path: string): UnityFileKind | undefined {
	const lowerPath = path.toLowerCase();
	if (lowerPath.endsWith(".unity")) return "scene";
	if (lowerPath.endsWith(".prefab")) return "prefab";
}

export function unityChangeSummary(changes: TreeChange[]) {
	const unityChanges = changes.filter((change) => isUnitySceneOrPrefabPath(change.path));
	return {
		total: unityChanges.length,
		scenes: unityChanges.filter((change) => unityFileKind(change.path) === "scene").length,
		prefabs: unityChanges.filter((change) => unityFileKind(change.path) === "prefab").length,
		firstPath: unityChanges[0]?.path,
	};
}
