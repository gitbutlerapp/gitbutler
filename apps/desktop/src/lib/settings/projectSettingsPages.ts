import { type IconName } from "@gitbutler/ui";

interface SettingsPage {
	id: string;
	label: string;
	icon: IconName;
	adminOnly?: boolean;
}

export const projectSettingsPages = [
	{
		id: "project",
		label: "Project",
		icon: "user",
	},
	{
		id: "git",
		label: "Git stuff",
		icon: "git",
	},
	{
		id: "ai",
		label: "AI options",
		icon: "ai",
	},
	{
		id: "agent",
		label: "Agent",
		icon: "agent",
	},
	{
		id: "experimental",
		label: "Experimental",
		icon: "lab",
	},
] as const satisfies readonly SettingsPage[];

export type ProjectSettingsPage = (typeof projectSettingsPages)[number];
// Canonical definition lives in state/uiState.svelte.ts to avoid circular imports.
export type { ProjectSettingsPageId } from "$lib/state/uiState.svelte";
