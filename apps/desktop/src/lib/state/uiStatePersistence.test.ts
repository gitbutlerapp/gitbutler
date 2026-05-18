import { sanitizePersistedUiStateKey } from "$lib/state/uiState.svelte";
import { describe, expect, test } from "vitest";

describe("sanitizePersistedUiStateKey", () => {
	test("drops transient project action ids from persisted UiState", () => {
		const result = sanitizePersistedUiStateKey("ids", [
			"defaultTerminal",
			"project-id:exclusiveAction",
			"project-id:stackBusy",
			"lane-id:selection",
		]);

		expect(result).toEqual(["defaultTerminal", "lane-id:selection"]);
	});

	test("drops transient project action entities from persisted UiState", () => {
		const result = sanitizePersistedUiStateKey("entities", {
			defaultTerminal: { id: "defaultTerminal", value: { identifier: "wt" } },
			"project-id:exclusiveAction": {
				id: "project-id:exclusiveAction",
				value: { type: "commit" },
			},
			"project-id:stackBusy": {
				id: "project-id:stackBusy",
				value: { stackIds: ["stack-id"] },
			},
			"lane-id:selection": {
				id: "lane-id:selection",
				value: { branchName: "main", previewOpen: true },
			},
		});

		expect(result).toEqual({
			defaultTerminal: { id: "defaultTerminal", value: { identifier: "wt" } },
			"lane-id:selection": {
				id: "lane-id:selection",
				value: { branchName: "main", previewOpen: true },
			},
		});
	});
});
