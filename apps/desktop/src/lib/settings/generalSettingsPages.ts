import { t } from "$lib/i18n";
import { type IconName } from "@gitbutler/ui-svelte";

interface SettingsPage {
	id: string;
	label: () => string;
	icon: IconName;
	adminOnly?: boolean;
}

export const generalSettingsPages = [
	{
		id: "general",
		label: () => t("general"),
		icon: "settings",
	},
	{
		id: "appearance",
		label: () => t("appearance"),
		icon: "appearance",
	},
	{
		id: "lanes-and-branches",
		label: () => t("lanes-branches"),
		icon: "lanes",
	},
	{
		id: "git",
		label: () => t("git-stuff"),
		icon: "git",
	},
	{
		id: "integrations",
		label: () => t("integrations"),
		icon: "puzzle",
	},
	{
		id: "ai",
		label: () => t("ai-options"),
		icon: "ai",
	},
	{
		id: "telemetry",
		label: () => t("telemetry"),
		icon: "chart-bar-x",
	},
	{
		id: "experimental",
		label: () => t("experimental"),
		icon: "lab",
	},
	{
		id: "organizations",
		label: () => t("organizations"),
		icon: "factory",
		adminOnly: true,
	},
] as const satisfies readonly SettingsPage[];

export type GeneralSettingsPage = (typeof generalSettingsPages)[number];
