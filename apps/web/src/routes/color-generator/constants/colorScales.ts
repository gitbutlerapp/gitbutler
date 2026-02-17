/**
 * Color scale constants and configuration
 */

import type { ColorScale, Shade, SemanticZone } from "../types/color";

export const SCALES: ColorScale[] = [
	{ name: "Gray", id: "gray", baseHue: 25 },
	{ name: "Pop (Teal)", id: "pop", baseHue: 180 },
	{ name: "Error (Red)", id: "danger", baseHue: 8 },
	{ name: "Warning (Orange)", id: "warn", baseHue: 35 },
	{ name: "Success (Green)", id: "safe", baseHue: 155 },
	{ name: "Purple", id: "purple", baseHue: 270 },
];

export const SHADES: Shade[] = [
	{ value: 100, purpose: "background" },
	{ value: 95, purpose: "background" },
	{ value: 90, purpose: "background" },
	{ value: 80, purpose: "soft" },
	{ value: 70, purpose: "soft" },
	{ value: 60, purpose: "soft" },
	{ value: 50, purpose: "solid" },
	{ value: 40, purpose: "solid" },
	{ value: 30, purpose: "text" },
	{ value: 20, purpose: "text" },
	{ value: 10, purpose: "text" },
	{ value: 5, purpose: "text" },
	{ value: 0, purpose: "text" },
];

export const SEMANTIC_ZONES: SemanticZone[] = [
	{ label: "Background", range: "100-90" },
	{ label: "Soft", range: "80-60" },
	{ label: "Solid", range: "50-40" },
	{ label: "Text", range: "30-0" },
];

export const DEFAULT_SATURATIONS: Record<string, number> = {
	gray: 8,
	pop: 65,
	safe: 60,
	danger: 68,
	warn: 95,
	purple: 60,
};

export const DEFAULT_SHADE_50_LIGHTNESS: Record<string, number> = {
	gray: 45,
	pop: 42,
	danger: 50,
	warn: 50,
	safe: 45,
	purple: 54,
};

export const ART_COLORS_LIGHT = {
	"art-scene-bg": { h: 175, s: 46, l: 89 },
	"art-scene-fill": { h: 60, s: 65, l: 97 },
	"art-scene-outline": { h: 180, s: 6, l: 30 },
};

export const ART_COLORS_DARK = {
	"art-scene-bg": { h: 177, s: 44, l: 28 },
	"art-scene-fill": { h: 79, s: 38, l: 82 },
	"art-scene-outline": { h: 180, s: 26, l: 11 },
};
