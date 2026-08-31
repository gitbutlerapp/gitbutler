import type { BundledTheme } from "shiki";
import type { GUISettings } from "#electron/settings.ts";

// [tag:lite_default_settings]
export const defaultSettings = {
	autoFetchFrequency: "15 min",
	autoUpdate: true,
	commentAnnotations: false,
	diffBackground: true,
	diffFontFamily: "Geist Mono, Menlo, monospace",
	diffFontSize: 12,
	diffLigatures: false,
	diffOverflow: "scroll",
	diffStyle: "split",
	diffTabSize: 4,
	// Previewing while dragging runs a dry run for every target the pointer crosses, and
	// each one takes the same workspace lock as the real operation. Off until that's cheap.
	dryRunOperations: false,
	// Show the folder tree until the user chooses a display mode.
	fileDisplayMode: "tree",
	// Pierre's own default, named here so the setting has somewhere to fall back to.
	lineDiffType: "word-alt",
	// Experimental; opt in from the Experimental settings.
	minimap: false,
	// Lite has always led with the file name; desktop leads with the path.
	pathFirst: false,
	// Loud = the notification bell and unread dots; quiet = dots only;
	// off = no PR-activity tracking in the UI at all.
	prNotifications: "loud",
	terminalId: "",
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
