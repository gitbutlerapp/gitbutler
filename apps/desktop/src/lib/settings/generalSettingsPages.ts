import { type IconName } from "@gitbutler/ui";

interface SettingsPage {
	id: string;
	label: string;
	icon: IconName;
	adminOnly?: boolean;
}

export const generalSettingsPages = [
	{
		id: "general",
		label: "General",
		icon: "settings",
	},
	{
		id: "appearance",
		label: "Appearance",
		icon: "appearance",
	},
	{
		id: "lanes-and-branches",
		label: "Lanes & branches",
		icon: "lanes",
	},
	{
		id: "git",
		label: "Git stuff",
		icon: "git",
	},
	{
		id: "integrations",
		label: "Integrations",
		icon: "puzzle",
	},
	{
		id: "ai",
		label: "AI Options",
		icon: "ai",
	},
	{
		id: "irc",
		label: "IRC",
		icon: "chat",
		adminOnly: true,
	},
	{
		id: "telemetry",
		label: "Telemetry",
		icon: "chart-bar-x",
	},
	{
		id: "experimental",
		label: "Experimental",
		icon: "lab",
	},
	{
		id: "organizations",
		label: "Organizations",
		icon: "factory",
		adminOnly: true,
	},
] as const satisfies readonly SettingsPage[];

export type GeneralSettingsPage = (typeof generalSettingsPages)[number];
export type GeneralSettingsPageId = GeneralSettingsPage["id"];
