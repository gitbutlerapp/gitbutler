import iconsJson from "@gitbutler/ui/data/icons.json";

export const projectSettingsPages = [
	{
		id: "project",
		label: "Project",
		icon: "profile" as keyof typeof iconsJson,
	},
	{
		id: "git",
		label: "Git stuff",
		icon: "git" as keyof typeof iconsJson,
	},
	{
		id: "ai",
		label: "AI options",
		icon: "ai" as keyof typeof iconsJson,
	},
	{
		id: "agent",
		label: "Agent",
		icon: "ai-agent" as keyof typeof iconsJson,
	},
	{
		id: "experimental",
		label: "Experimental",
		icon: "idea" as keyof typeof iconsJson,
	},
] as const;

export type ProjectSettingsPage = (typeof projectSettingsPages)[number];
export type ProjectSettingsPageId = ProjectSettingsPage["id"];
