import type { IconName } from "#ui/components/iconNames.ts";

/** What a page configures: the application, or the project it was opened over. */
type SettingsScope = "global" | "project";

type SettingsPage = {
	/**
	 * Scope-prefixed, which is what keeps a page name that exists in both scopes — `git`
	 * will — from colliding. The prefix is the identity, so there is no separate scope
	 * field to fall out of step with it.
	 */
	key: `${SettingsScope}:${string}`;
	label: string;
	icon: IconName;
};

/**
 * Every settings page, in sidebar order, both scopes in one list.
 *
 * Desktop needs two dialogs for this because its global settings are reachable without a
 * project. Lite's are not — the dialog only mounts inside the workspace route — so one
 * list carries both, and a project page is a page like any other.
 */
export const settingsPages = [
	{ key: "global:general", label: "General", icon: "settings" },
	{ key: "global:appearance", label: "Appearance", icon: "mixer" },
	{ key: "global:ai", label: "AI", icon: "ai" },
	{ key: "global:git", label: "Git", icon: "branch" },
	{ key: "global:integrations", label: "Integrations", icon: "globe" },
	{ key: "project:project", label: "Project", icon: "workbench" },
	{ key: "project:ai", label: "AI", icon: "ai" },
	{ key: "project:git", label: "Git", icon: "branch" },
	{ key: "project:experimental", label: "Experimental", icon: "danger" },
] as const satisfies ReadonlyArray<SettingsPage>;

/** Sidebar group order. */
export const settingsScopes = ["global", "project"] as const satisfies ReadonlyArray<SettingsScope>;

export type SettingsPageKey = (typeof settingsPages)[number]["key"];

/** Sits under the pages, the way desktop ends its settings nav. */
export const externalLinks = [
	{ label: "Docs", icon: "docs", url: "https://docs.gitbutler.com/" },
	{ label: "Our Discord", icon: "discord", url: "https://discord.gg/MmFkmaJ42D" },
] as const satisfies ReadonlyArray<{ label: string; icon: IconName; url: string }>;

export const settingsPagesInScope = (scope: SettingsScope) =>
	settingsPages.filter((page) => page.key.startsWith(`${scope}:`));

export const defaultSettingsPageKey = "global:general" satisfies SettingsPageKey;
