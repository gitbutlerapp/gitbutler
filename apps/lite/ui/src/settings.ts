import type { BundledTheme } from "shiki";
import type { GUISettings } from "#electron/settings.ts";

export const defaultSettings = {
	diffBackground: true,
	diffFontFamily: "Geist Mono, Menlo, monospace",
	diffFontSize: 12,
	diffOverflow: "scroll",
	diffStyle: "unified",
	diffTabSize: 4,
	filesVisible: false,
	// Pierre doesn't re-export BundledTheme from Shiki and it's not possible to extract it from the
	// union, hence importing from Shiki. See also:
	//   https://shiki.style/themes#bundled-themes
	syntaxHighlighting: {
		light: "github-light-default" satisfies BundledTheme,
		dark: "github-dark-default" satisfies BundledTheme,
	},
} satisfies Partial<GUISettings>;
