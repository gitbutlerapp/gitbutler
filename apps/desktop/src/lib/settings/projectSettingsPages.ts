import { t } from "$lib/i18n";
import { type IconName } from "@gitbutler/ui-svelte";

interface SettingsPage {
	id: string;
	label: () => string;
	icon: IconName;
	adminOnly?: boolean;
}

export const projectSettingsPages = [
	{
		id: "project",
		label: () => t("project"),
		icon: "user",
	},
	{
		id: "git",
		label: () => t("git-stuff"),
		icon: "git",
	},
	{
		id: "ai",
		label: () => t("ai-options"),
		icon: "ai",
	},
	{
		id: "experimental",
		label: () => t("experimental"),
		icon: "lab",
	},
] as const satisfies readonly SettingsPage[];

export type ProjectSettingsPage = (typeof projectSettingsPages)[number];
