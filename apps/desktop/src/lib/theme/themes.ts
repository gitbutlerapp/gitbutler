export type ThemeScheme = "light" | "dark";

export type AppTheme =
	| "system"
	| "light"
	| "dark"
	| "ayu-ember"
	| "moonlight"
	| "poimandres"
	| "lavi"
	| "tokyo-night"
	| "catppuccin-latte"
	| "catppuccin-frappe"
	| "catppuccin-macchiato"
	| "catppuccin-mocha";

type ThemeTokenOverrides = Record<`--${string}`, string>;

type ThemePreview = {
	page: string;
	sidebar: string;
	panel: string;
	element: string;
	accent: string;
	text: string;
};

type ThemeDefinition = {
	id: AppTheme;
	label: string;
	scheme: ThemeScheme | "system";
	preview: ThemePreview;
};

type ThemePalette = {
	background: string;
	panel: string;
	element: string;
	border: string;
	text: string;
	muted: string;
	primary: string;
	secondary: string;
	success: string;
	warning: string;
	danger: string;
	info: string;
	deep?: string;
	subtle?: string;
	accentOn?: string;
};

function mix(color: string, background: string, amount: number) {
	return `color-mix(in srgb, ${color} ${amount}%, ${background})`;
}

function buildDarkScale(name: "pop" | "purple" | "safe" | "danger" | "warn", color: string) {
	return {
		[`--clr-${name}-5`]: mix(color, "var(--clr-gray-5)", 18),
		[`--clr-${name}-10`]: mix(color, "var(--clr-gray-5)", 24),
		[`--clr-${name}-20`]: mix(color, "var(--clr-gray-5)", 34),
		[`--clr-${name}-30`]: mix(color, "var(--clr-gray-5)", 46),
		[`--clr-${name}-40`]: mix(color, "var(--clr-gray-5)", 62),
		[`--clr-${name}-50`]: color,
		[`--clr-${name}-60`]: mix(color, "var(--clr-gray-95)", 74),
		[`--clr-${name}-70`]: mix(color, "var(--clr-gray-95)", 56),
		[`--clr-${name}-80`]: mix(color, "var(--clr-gray-95)", 34),
		[`--clr-${name}-90`]: mix(color, "var(--clr-gray-95)", 18),
		[`--clr-${name}-95`]: mix(color, "var(--clr-gray-95)", 10),
	} satisfies ThemeTokenOverrides;
}

function buildLightScale(name: "pop" | "purple" | "safe" | "danger" | "warn", color: string) {
	return {
		[`--clr-${name}-5`]: mix(color, "var(--clr-gray-5)", 78),
		[`--clr-${name}-10`]: mix(color, "var(--clr-gray-5)", 66),
		[`--clr-${name}-20`]: mix(color, "var(--clr-gray-5)", 54),
		[`--clr-${name}-30`]: mix(color, "var(--clr-gray-5)", 44),
		[`--clr-${name}-40`]: mix(color, "var(--clr-gray-5)", 32),
		[`--clr-${name}-50`]: color,
		[`--clr-${name}-60`]: mix(color, "var(--clr-gray-90)", 62),
		[`--clr-${name}-70`]: mix(color, "var(--clr-gray-90)", 44),
		[`--clr-${name}-80`]: mix(color, "var(--clr-gray-90)", 28),
		[`--clr-${name}-90`]: mix(color, "var(--clr-gray-90)", 16),
		[`--clr-${name}-95`]: mix(color, "var(--clr-gray-90)", 10),
	} satisfies ThemeTokenOverrides;
}

function createDarkThemeTokens(palette: ThemePalette): ThemeTokenOverrides {
	const subtle = palette.subtle ?? mix(palette.text, palette.background, 54);

	return {
		"--bg-overlay": "rgb(0 0 0 / 0.56)",
		"--shadow-clr": "rgb(0 0 0 / 0.46)",
		"--clr-gray-0": palette.deep ?? mix(palette.background, "#000000", 72),
		"--clr-gray-5": palette.background,
		"--clr-gray-10": palette.panel,
		"--clr-gray-20": palette.element,
		"--clr-gray-30": mix(palette.border, palette.element, 82),
		"--clr-gray-40": palette.border,
		"--clr-gray-50": subtle,
		"--clr-gray-60": palette.muted,
		"--clr-gray-70": mix(palette.text, palette.background, 58),
		"--clr-gray-80": mix(palette.text, palette.background, 74),
		"--clr-gray-90": mix(palette.text, palette.background, 88),
		"--clr-gray-95": palette.text,
		"--clr-gray-100": "#ffffff",
		...buildDarkScale("pop", palette.primary),
		...buildDarkScale("purple", palette.secondary),
		...buildDarkScale("safe", palette.success),
		...buildDarkScale("danger", palette.danger),
		...buildDarkScale("warn", palette.warning),
		"--diff-addition-line-bg": mix(palette.success, palette.background, 18),
		"--diff-addition-line-highlight": mix(palette.success, palette.background, 34),
		"--diff-addition-count-bg": mix(palette.success, palette.background, 26),
		"--diff-addition-count-border": mix(palette.success, palette.background, 42),
		"--diff-addition-count-text": mix(palette.success, palette.text, 72),
		"--diff-deletion-line-bg": mix(palette.danger, palette.background, 18),
		"--diff-deletion-line-highlight": mix(palette.danger, palette.background, 34),
		"--diff-deletion-count-bg": mix(palette.danger, palette.background, 26),
		"--diff-deletion-count-border": mix(palette.danger, palette.background, 42),
		"--diff-deletion-count-text": mix(palette.danger, palette.text, 72),
		"--diff-count-bg": mix(palette.text, palette.background, 12),
		"--diff-count-border": palette.border,
		"--diff-count-text": palette.muted,
		"--diff-line-bg": palette.panel,
		"--art-scene-bg": mix(palette.primary, palette.background, 24),
		"--art-scene-fill": mix(palette.text, palette.background, 80),
		"--art-scene-outline": mix(palette.text, palette.background, 18),
		"--art-spot-fill": mix(palette.text, palette.background, 44),
		"--art-spot-fill-pop": mix(palette.primary, palette.background, 56),
	};
}

function createLightThemeTokens(palette: ThemePalette): ThemeTokenOverrides {
	const subtle = palette.subtle ?? mix(palette.text, palette.background, 36);

	return {
		"--bg-overlay": "rgb(255 255 255 / 0.5)",
		"--shadow-clr": "rgb(0 0 0 / 0.12)",
		"--clr-gray-0": "#000000",
		"--clr-gray-5": palette.text,
		"--clr-gray-10": mix(palette.text, palette.background, 82),
		"--clr-gray-20": palette.muted,
		"--clr-gray-30": subtle,
		"--clr-gray-40": palette.border,
		"--clr-gray-50": mix(palette.border, palette.background, 72),
		"--clr-gray-60": mix(palette.border, palette.background, 58),
		"--clr-gray-70": palette.element,
		"--clr-gray-80": palette.panel,
		"--clr-gray-90": palette.background,
		"--clr-gray-95": mix("#ffffff", palette.background, 78),
		"--clr-gray-100": "#ffffff",
		...buildLightScale("pop", palette.primary),
		...buildLightScale("purple", palette.secondary),
		...buildLightScale("safe", palette.success),
		...buildLightScale("danger", palette.danger),
		...buildLightScale("warn", palette.warning),
		"--diff-addition-line-bg": mix(palette.success, palette.background, 18),
		"--diff-addition-line-highlight": mix(palette.success, palette.background, 28),
		"--diff-addition-count-bg": mix(palette.success, palette.background, 24),
		"--diff-addition-count-border": mix(palette.success, palette.background, 42),
		"--diff-addition-count-text": mix(palette.success, palette.text, 60),
		"--diff-deletion-line-bg": mix(palette.danger, palette.background, 18),
		"--diff-deletion-line-highlight": mix(palette.danger, palette.background, 28),
		"--diff-deletion-count-bg": mix(palette.danger, palette.background, 24),
		"--diff-deletion-count-border": mix(palette.danger, palette.background, 42),
		"--diff-deletion-count-text": mix(palette.danger, palette.text, 60),
		"--diff-count-bg": mix(palette.text, palette.background, 4),
		"--diff-count-border": palette.border,
		"--diff-count-text": palette.muted,
		"--diff-line-bg": palette.element,
		"--art-scene-bg": mix(palette.primary, palette.background, 16),
		"--art-scene-fill": mix(palette.text, palette.background, 6),
		"--art-scene-outline": mix(palette.text, palette.background, 22),
		"--art-spot-fill": mix(palette.text, palette.background, 54),
		"--art-spot-fill-pop": mix(palette.primary, palette.background, 64),
	};
}

const themeOptions: ThemeDefinition[] = [
	{
		id: "system",
		label: "System preference",
		scheme: "system",
		preview: {
			page: "#f4f4f4",
			sidebar: "#ffffff",
			panel: "#303030",
			element: "#565656",
			accent: "#41b4ae",
			text: "#111111",
		},
	},
	{
		id: "light",
		label: "Light",
		scheme: "light",
		preview: {
			page: "#f4f4f4",
			sidebar: "#ffffff",
			panel: "#e6e6e6",
			element: "#ffffff",
			accent: "#41b4ae",
			text: "#111111",
		},
	},
	{
		id: "dark",
		label: "Dark",
		scheme: "dark",
		preview: {
			page: "#565656",
			sidebar: "#303030",
			panel: "#252525",
			element: "#303030",
			accent: "#77b7b3",
			text: "#f4f4f4",
		},
	},
	{
		id: "ayu-ember",
		label: "Ayu Ember",
		scheme: "dark",
		preview: {
			page: "#0d1017",
			sidebar: "#141821",
			panel: "#10141c",
			element: "#1b1f29",
			accent: "#e6b450",
			text: "#bfbdb6",
		},
	},
	{
		id: "moonlight",
		label: "Moonlight",
		scheme: "dark",
		preview: {
			page: "#222436",
			sidebar: "#191a2a",
			panel: "#1e2030",
			element: "#2f334d",
			accent: "#3ad7c7",
			text: "#c8d3f5",
		},
	},
	{
		id: "poimandres",
		label: "Poimandres",
		scheme: "dark",
		preview: {
			page: "#1b1e28",
			sidebar: "#1f2330",
			panel: "#303340",
			element: "#262b38",
			accent: "#5de4c7",
			text: "#e4f0fb",
		},
	},
	{
		id: "lavi",
		label: "Lavi",
		scheme: "dark",
		preview: {
			page: "#25213b",
			sidebar: "#312a4d",
			panel: "#3b355f",
			element: "#463e6f",
			accent: "#eab9f9",
			text: "#ede7fe",
		},
	},
	{
		id: "tokyo-night",
		label: "Tokyo Night",
		scheme: "dark",
		preview: {
			page: "#1a1b26",
			sidebar: "#16161e",
			panel: "#24283b",
			element: "#2a2f45",
			accent: "#86e1fc",
			text: "#c8d3f5",
		},
	},
	{
		id: "catppuccin-latte",
		label: "Catppuccin Latte",
		scheme: "light",
		preview: {
			page: "#eff1f5",
			sidebar: "#e6e9ef",
			panel: "#dce0e8",
			element: "#ffffff",
			accent: "#1e66f5",
			text: "#4c4f69",
		},
	},
	{
		id: "catppuccin-frappe",
		label: "Catppuccin Frappe",
		scheme: "dark",
		preview: {
			page: "#303446",
			sidebar: "#292c3c",
			panel: "#232634",
			element: "#414559",
			accent: "#8caaee",
			text: "#c6d0f5",
		},
	},
	{
		id: "catppuccin-macchiato",
		label: "Catppuccin Macchiato",
		scheme: "dark",
		preview: {
			page: "#24273a",
			sidebar: "#1e2030",
			panel: "#181926",
			element: "#363a4f",
			accent: "#8aadf4",
			text: "#cad3f5",
		},
	},
	{
		id: "catppuccin-mocha",
		label: "Catppuccin Mocha",
		scheme: "dark",
		preview: {
			page: "#1e1e2e",
			sidebar: "#181825",
			panel: "#11111b",
			element: "#313244",
			accent: "#89b4fa",
			text: "#cdd6f4",
		},
	},
];

const themeTokensById: Partial<Record<AppTheme, ThemeTokenOverrides>> = {
	"ayu-ember": createDarkThemeTokens({
		background: "#0d1017",
		panel: "#141821",
		element: "#10141c",
		border: "#1b1f29",
		text: "#bfbdb6",
		muted: "#6c7380",
		primary: "#e6b450",
		secondary: "#d2a6ff",
		success: "#70bf56",
		warning: "#ff8f40",
		danger: "#d95757",
		info: "#39bae6",
	}),
	moonlight: createDarkThemeTokens({
		background: "#222436",
		panel: "#1e2030",
		element: "#2f334d",
		border: "#444a73",
		text: "#c8d3f5",
		muted: "#828bb8",
		primary: "#3ad7c7",
		secondary: "#c4a2ff",
		success: "#c3e88d",
		warning: "#ffc777",
		danger: "#ff757f",
		info: "#78dbff",
	}),
	poimandres: createDarkThemeTokens({
		background: "#1b1e28",
		panel: "#232835",
		element: "#303340",
		border: "#506477",
		text: "#e4f0fb",
		muted: "#a6accd",
		primary: "#5de4c7",
		secondary: "#89ddff",
		success: "#00ced1",
		warning: "#fffac2",
		danger: "#d0679d",
		info: "#add7ff",
	}),
	lavi: createDarkThemeTokens({
		background: "#25213b",
		panel: "#3b355f",
		element: "#463e6f",
		border: "#4c425c",
		text: "#ede7fe",
		muted: "#a79bcf",
		primary: "#af8eeb",
		secondary: "#7583ff",
		success: "#7cf89b",
		warning: "#ff9a6b",
		danger: "#f47189",
		info: "#80bdff",
	}),
	"tokyo-night": createDarkThemeTokens({
		background: "#1a1b26",
		panel: "#24283b",
		element: "#2a2f45",
		border: "#545c7e",
		text: "#c8d3f5",
		muted: "#828bb8",
		primary: "#82aaff",
		secondary: "#c099ff",
		success: "#c3e88d",
		warning: "#ff966c",
		danger: "#ff757f",
		info: "#86e1fc",
	}),
	"catppuccin-latte": createLightThemeTokens({
		background: "#eff1f5",
		panel: "#e6e9ef",
		element: "#dce0e8",
		border: "#ccd0da",
		text: "#4c4f69",
		muted: "#5c5f77",
		primary: "#1e66f5",
		secondary: "#ea76cb",
		success: "#40a02b",
		warning: "#df8e1d",
		danger: "#d20f39",
		info: "#179299",
	}),
	"catppuccin-frappe": createDarkThemeTokens({
		background: "#303446",
		panel: "#292c3c",
		element: "#232634",
		border: "#414559",
		text: "#c6d0f5",
		muted: "#b5bfe2",
		primary: "#8caaee",
		secondary: "#f4b8e4",
		success: "#a6d189",
		warning: "#e5c890",
		danger: "#e78284",
		info: "#81c8be",
	}),
	"catppuccin-macchiato": createDarkThemeTokens({
		background: "#24273a",
		panel: "#1e2030",
		element: "#181926",
		border: "#363a4f",
		text: "#cad3f5",
		muted: "#b8c0e0",
		primary: "#8aadf4",
		secondary: "#f5bde6",
		success: "#a6da95",
		warning: "#eed49f",
		danger: "#ed8796",
		info: "#8bd5ca",
	}),
	"catppuccin-mocha": createDarkThemeTokens({
		background: "#1e1e2e",
		panel: "#181825",
		element: "#11111b",
		border: "#313244",
		text: "#cdd6f4",
		muted: "#bac2de",
		primary: "#89b4fa",
		secondary: "#f5c2e7",
		success: "#a6e3a1",
		warning: "#f9e2af",
		danger: "#f38ba8",
		info: "#94e2d5",
	}),
};

const themeLookup = new Map(themeOptions.map((theme) => [theme.id, theme]));
const themeTokenKeys = [
	...new Set(Object.values(themeTokensById).flatMap((theme) => Object.keys(theme ?? {}))),
];

function getPreferredScheme(): ThemeScheme {
	if (typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches) {
		return "dark";
	}

	return "light";
}

function isThemeScheme(value: string | null | undefined): value is ThemeScheme {
	return value === "light" || value === "dark";
}

export const THEME_OPTIONS = themeOptions;
export const QUICK_THEME_OPTIONS = themeOptions.filter((theme) =>
	["system", "light", "dark", "ayu-ember", "moonlight", "poimandres", "catppuccin-mocha"].includes(
		theme.id,
	),
);

export function getThemeDefinition(themeId: AppTheme) {
	return themeLookup.get(themeId) ?? themeLookup.get("dark")!;
}

export function getResolvedThemeId(
	selectedTheme: AppTheme | undefined,
	systemTheme: string | null | undefined,
): Exclude<AppTheme, "system"> {
	if (selectedTheme && selectedTheme !== "system") {
		return selectedTheme;
	}

	return isThemeScheme(systemTheme) ? systemTheme : getPreferredScheme();
}

export function getResolvedThemeScheme(
	selectedTheme: AppTheme | undefined,
	systemTheme: string | null | undefined,
) {
	return getThemeDefinition(getResolvedThemeId(selectedTheme, systemTheme)).scheme as ThemeScheme;
}

export function applyThemeToDocument(
	docEl: HTMLElement,
	selectedTheme: AppTheme | undefined,
	systemTheme: string | null | undefined,
) {
	const resolvedThemeId = getResolvedThemeId(selectedTheme, systemTheme);
	const resolvedTheme = getThemeDefinition(resolvedThemeId);

	docEl.dataset.theme = selectedTheme ?? "system";
	docEl.dataset.resolvedTheme = resolvedTheme.id;
	docEl.classList.remove("light", "dark");
	docEl.classList.add(resolvedTheme.scheme);
	docEl.style.colorScheme = resolvedTheme.scheme;

	for (const tokenKey of themeTokenKeys) {
		docEl.style.removeProperty(tokenKey);
	}

	for (const [tokenKey, value] of Object.entries(themeTokensById[resolvedThemeId] ?? {})) {
		docEl.style.setProperty(tokenKey, value);
	}
}
