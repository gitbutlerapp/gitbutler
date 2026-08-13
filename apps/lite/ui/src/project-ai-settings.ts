import { queryOptions } from "@tanstack/react-query";

export const DEFAULT_COMMIT_MESSAGE_PROMPT = `Write a commit message for these changes.
Only respond with the commit message, without notes or Markdown.
Explain what changed and why, focusing on the most important changes.
Use present tense and a semantic commit prefix.
Keep the title under 50 characters and hard-wrap body lines at 72 characters.
Do not use emoji or start lines with the hash symbol.`;

export type ProjectAiSettings = {
	enabled: boolean;
	commitMessagePrompt: string;
};

const defaultProjectAiSettings: ProjectAiSettings = {
	enabled: false,
	commitMessagePrompt: DEFAULT_COMMIT_MESSAGE_PROMPT,
};

const storageKey = (projectId: string) => `project_ai_settings:v1:${projectId}`;

const readProjectAiSettings = (projectId: string): ProjectAiSettings => {
	try {
		const stored: unknown = JSON.parse(
			window.localStorage.getItem(storageKey(projectId)) ?? "null",
		);
		if (
			typeof stored === "object" &&
			stored !== null &&
			"enabled" in stored &&
			"commitMessagePrompt" in stored
		) {
			const { enabled, commitMessagePrompt } = stored;
			if (typeof enabled === "boolean" && typeof commitMessagePrompt === "string") {
				return {
					enabled,
					commitMessagePrompt:
						commitMessagePrompt.trim().length > 0
							? commitMessagePrompt.trim()
							: DEFAULT_COMMIT_MESSAGE_PROMPT,
				};
			}
		}
	} catch {
		// Invalid renderer-owned settings fall back to defaults.
	}
	return defaultProjectAiSettings;
};

export const writeProjectAiSettings = (projectId: string, settings: ProjectAiSettings): void =>
	window.localStorage.setItem(storageKey(projectId), JSON.stringify(settings));

export const projectAiSettingsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["projectAiSettings", projectId],
		queryFn: () => readProjectAiSettings(projectId),
	});
