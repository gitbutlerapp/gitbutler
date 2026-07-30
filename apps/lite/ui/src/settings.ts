import type { BundledTheme } from "shiki";
import type { GUISettings } from "#electron/settings.ts";

// [tag:lite_default_settings]
export const defaultSettings = {
	autoFetchFrequency: "15 min",
	diffBackground: true,
	diffFontFamily: "Geist Mono, Menlo, monospace",
	diffFontSize: 12,
	diffOverflow: "scroll",
	diffStyle: "unified",
	diffTabSize: 4,
	// Pierre doesn't re-export BundledTheme from Shiki and it's not possible to extract it from the
	// union, hence importing from Shiki. See also:
	//   https://shiki.style/themes#bundled-themes
	syntaxHighlighting: {
		light: "github-light-default" satisfies BundledTheme,
		dark: "github-dark-default" satisfies BundledTheme,
	},
	theme: "system",
	unidiff: true,
} satisfies Partial<GUISettings>;

export const clampAutoFetch = (ms: number): number =>
	Math.min(Math.max(ms, 10_000), 60 * 1000 * 60 * 24);
