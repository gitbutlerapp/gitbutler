import type { TreeChange, TreeStatus } from "@gitbutler/but-sdk";

export function isTreeChange(something: unknown): something is TreeChange {
	return (
		typeof something === "object" &&
		something !== null &&
		"path" in something &&
		typeof something["path"] === "string" &&
		"pathBytes" in something &&
		Array.isArray(something["pathBytes"]) &&
		"status" in something &&
		isChangeStatus(something["status"])
	);
}

export function isExecutableStatus(status: TreeStatus): boolean {
	switch (status.type) {
		case "Addition":
		case "Deletion":
			return false;
		case "Modification":
		case "Rename":
			return (
				status.subject.flags === "ExecutableBitAdded" ||
				status.subject.flags === "ExecutableBitRemoved"
			);
	}
}

export function isSubmoduleStatus(status: TreeStatus): boolean {
	switch (status.type) {
		case "Addition":
			return status.subject.state.kind === "Commit";
		case "Deletion":
			return status.subject.previousState.kind === "Commit";
		case "Modification":
			return (
				status.subject.state.kind === "Commit" || status.subject.previousState.kind === "Commit"
			);
		case "Rename":
			return (
				status.subject.state.kind === "Commit" || status.subject.previousState.kind === "Commit"
			);
	}
}

function isChangeStatus(something: unknown): something is TreeStatus {
	return (
		typeof something === "object" &&
		something !== null &&
		"type" in something &&
		typeof something["type"] === "string"
	);
}
