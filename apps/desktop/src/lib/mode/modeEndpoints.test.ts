import { buildModeEndpoints } from "$lib/mode/modeEndpoints";
import { describe, expect, test } from "vitest";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

function createEndpointBuilder(): BackendEndpointBuilder {
	return {
		mutation: (definition) => definition,
		query: (definition) => definition,
	} as BackendEndpointBuilder;
}

describe("buildModeEndpoints", () => {
	test("names the enter-edit-mode mutation", () => {
		const endpoints = buildModeEndpoints(createEndpointBuilder());

		expect(endpoints.enterEditMode.extraOptions).toEqual({
			command: "enter_edit_mode",
			actionName: "Enter Edit Mode",
		});
	});
});
