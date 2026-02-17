import iconsJson from "@gitbutler/ui/data/icons.json";

export const generalSettingsPages = [
	{
		id: "general",
		label: "General",
		icon: "settings" as keyof typeof iconsJson,
	},
	{
		id: "appearance",
		label: "Appearance",
		icon: "appearance" as keyof typeof iconsJson,
	},
	{
		id: "lanes-and-branches",
		label: "Lanes & branches",
		icon: "lanes" as keyof typeof iconsJson,
	},
	{
		id: "git",
		label: "Git stuff",
		icon: "git" as keyof typeof iconsJson,
	},
	{
		id: "integrations",
		label: "Integrations",
		icon: "integrations" as keyof typeof iconsJson,
	},
	{
		id: "ai",
		label: "AI Options",
		icon: "ai" as keyof typeof iconsJson,
	},
	{
		id: "telemetry",
		label: "Telemetry",
		icon: "stat" as keyof typeof iconsJson,
	},
	{
		id: "experimental",
		label: "Experimental",
		icon: "idea" as keyof typeof iconsJson,
	},
	{
		id: "organizations",
		label: "Organizations",
		icon: "idea" as keyof typeof iconsJson,
		adminOnly: true,
	},
] as const;

export type GeneralSettingsPage = (typeof generalSettingsPages)[number];
export type GeneralSettingsPageId = GeneralSettingsPage["id"];
