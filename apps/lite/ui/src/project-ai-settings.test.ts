/** @vitest-environment jsdom */

import { beforeEach, describe, expect, it } from "vitest";
import {
	DEFAULT_COMMIT_MESSAGE_PROMPT,
	projectAiSettingsQueryOptions,
	writeProjectAiSettings,
} from "./project-ai-settings.ts";

const readProjectAiSettings = async (projectId: string) => {
	const queryFn = projectAiSettingsQueryOptions(projectId).queryFn;
	if (typeof queryFn !== "function") throw new Error("Missing project AI settings query");
	return await queryFn({} as never);
};

describe("project AI settings", () => {
	beforeEach(() => window.localStorage.clear());

	it("defaults to disabled with the commit prompt", async () => {
		await expect(readProjectAiSettings("project")).resolves.toEqual({
			enabled: false,
			commitMessagePrompt: DEFAULT_COMMIT_MESSAGE_PROMPT,
		});
	});

	it("persists per project and restores an empty prompt", async () => {
		writeProjectAiSettings("one", { enabled: true, commitMessagePrompt: "Custom" });
		writeProjectAiSettings("two", { enabled: false, commitMessagePrompt: "  " });

		await expect(readProjectAiSettings("one")).resolves.toEqual({
			enabled: true,
			commitMessagePrompt: "Custom",
		});
		await expect(readProjectAiSettings("two")).resolves.toMatchObject({
			commitMessagePrompt: DEFAULT_COMMIT_MESSAGE_PROMPT,
		});
	});
});
