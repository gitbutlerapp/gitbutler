import { findPage, loadPage, pages, type Page } from "./pages.ts";
import presets from "@gitbutler/but-ui/presets";
import type { MouseEvent } from "react";

/** The page the address names, or the design overview. */
export const currentPage = (): Page | undefined =>
	findPage(new URLSearchParams(window.location.search).get("page")) ?? pages[0];

const listeners = new Set<() => void>();
export const subscribePage = (listener: () => void) => {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
};
const showPage = () => {
	for (const listener of listeners) listener();
};
window.addEventListener("popstate", showPage);

export const pageHref = (id: string) => `?page=${id}`;

/**
 * A link to a page. A plain click loads the page first and then swaps it in, so the one on
 * screen stays until the next is ready; a click that opens a new tab goes where the address says.
 */
export const pageLink = (id: string) => ({
	href: pageHref(id),
	onClick: (event: MouseEvent<HTMLAnchorElement>) => {
		const page = findPage(id);
		if (
			page === undefined ||
			event.button !== 0 ||
			event.metaKey ||
			event.ctrlKey ||
			event.shiftKey ||
			event.altKey
		)
			return;
		event.preventDefault();
		void loadPage(page).then(() => {
			window.history.pushState(null, "", pageHref(id));
			document.querySelector(".docs-site-main")?.scrollTo(0, 0);
			showPage();
		});
	},
});

export type Theme = "system" | "light" | "dark";

/**
 * The theme is the root's `light` or `dark` class, which the tokens follow; without either they
 * follow the system's.
 */
export const paintTheme = (theme: Theme, root = document.documentElement) => {
	root.classList.toggle("dark", theme === "dark");
	root.classList.toggle("light", theme === "light");
};

const themeKey = "ui-react-docs-theme";

/** The theme last picked here, else the system's. */
export function savedTheme(): Theme {
	try {
		const saved = window.localStorage.getItem(themeKey);
		if (saved === "light" || saved === "dark") return saved;
	} catch {
		// Storage can be off; the system's theme stands in.
	}
	return "system";
}

/** A named accent, the ramps the tokens point the primary family at, or any colour as a hex. */
export type Accent = "teal" | "blue" | "purple" | `#${string}`;

/** `value` as an accent: a named one, or a six-digit hex with or without its `#`; else teal. */
export const accentOf = (value: string | null | undefined): Accent => {
	if (value === "blue" || value === "purple") return value;
	const hex = value?.match(/^#?([0-9a-f]{6})$/i)?.[1];
	return hex === undefined ? "teal" : `#${hex.toUpperCase()}`;
};

/**
 * The accent is the root's `data-but-accent`, which the tokens follow; without it, teal. Any
 * colour is `custom`, with the colour as `--but-accent-color` beside it.
 */
export const paintAccent = (accent: Accent, root = document.documentElement) => {
	const custom = accent.startsWith("#");
	if (accent === "teal") delete root.dataset["butAccent"];
	else root.dataset["butAccent"] = custom ? "custom" : accent;
	if (custom) root.style.setProperty("--but-accent-color", accent);
	else root.style.removeProperty("--but-accent-color");
};

const accentKey = "ui-react-docs-accent";

/** The accent last picked here, else teal. */
export function savedAccent(): Accent {
	try {
		return accentOf(window.localStorage.getItem(accentKey));
	} catch {
		// Storage can be off; teal stands in.
		return "teal";
	}
}

export function pickAccent(accent: Accent) {
	paintAccent(accent);
	try {
		window.localStorage.setItem(accentKey, accent);
	} catch {
		// Kept for this visit only.
	}
}

/** The corners: soft, Figma's, or the whole scale 4px sharper or rounder. */
export type Radius = "sharp" | "soft" | "round";

/** `value` as a corner setting; else Figma's. */
export const radiusOf = (value: string | null | undefined): Radius =>
	value === "sharp" || value === "round" ? value : "soft";

/** The corners are the root's `data-but-radius`, which the tokens follow; without it, Figma's. */
export const paintRadius = (radius: Radius, root = document.documentElement) => {
	if (radius === "soft") delete root.dataset["butRadius"];
	else root.dataset["butRadius"] = radius;
};

const radiusKey = "ui-react-docs-radius";

/** The corners last picked here, else Figma's. */
export function savedRadius(): Radius {
	try {
		return radiusOf(window.localStorage.getItem(radiusKey));
	} catch {
		// Storage can be off; Figma's stand in.
		return "soft";
	}
}

export function pickRadius(radius: Radius) {
	paintRadius(radius);
	try {
		window.localStorage.setItem(radiusKey, radius);
	} catch {
		// Kept for this visit only.
	}
}

/** The heights: balanced, Figma's, or a step shorter or taller. */
export type Density = "compact" | "balanced" | "spacious";

/** `value` as a density; else Figma's. */
export const densityOf = (value: string | null | undefined): Density =>
	value === "compact" || value === "spacious" ? value : "balanced";

/** The density is the root's `data-but-density`, which the tokens follow; without it, Figma's. */
export const paintDensity = (density: Density, root = document.documentElement) => {
	if (density === "balanced") delete root.dataset["butDensity"];
	else root.dataset["butDensity"] = density;
};

const densityKey = "ui-react-docs-density";

/** The density last picked here, else Figma's. */
export function savedDensity(): Density {
	try {
		return densityOf(window.localStorage.getItem(densityKey));
	} catch {
		// Storage can be off; Figma's stands in.
		return "balanced";
	}
}

export function pickDensity(density: Density) {
	paintDensity(density);
	try {
		window.localStorage.setItem(densityKey, density);
	} catch {
		// Kept for this visit only.
	}
}

/** A named pair of corners and density, from but-ui; colour and theme stay as picked. */
export type Preset = { id: string; name: string; radius: Radius; density: Density };

export const PRESETS: Array<Preset> = presets.map((preset) => ({
	...preset,
	radius: radiusOf(preset.radius),
	density: densityOf(preset.density),
}));

/** The preset these settings make, if any. */
export const presetOf = (radius: Radius, density: Density): Preset | undefined =>
	PRESETS.find((preset) => preset.radius === radius && preset.density === density);

/** Picks both of a preset's settings, as picking them one at a time would. */
export function pickPreset(preset: Preset) {
	pickRadius(preset.radius);
	pickDensity(preset.density);
}

export function pickTheme(theme: Theme) {
	paintTheme(theme);
	try {
		window.localStorage.setItem(themeKey, theme);
	} catch {
		// Kept for this visit only.
	}
}
