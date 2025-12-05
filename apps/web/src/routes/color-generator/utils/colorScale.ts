/**
 * Color scale generation utilities with perceptual easing
 *
 * The scale is divided into 4 semantic zones:
 * - Background (100-80): Subtle backgrounds, nearly white/black
 * - Soft (70-50): Interactive elements, borders, muted colors
 * - Solid (40-20): Primary interactive elements, buttons, strong accents
 * - Text (10-0): High contrast text, icons
 */

import { hslToRgb, rgbToHex } from './colorConversion';
import type { ColorScale, Shade } from '../types/color';

// Semantic zone definitions
export const ZONES = {
	background: { range: [100, 80], description: 'Subtle backgrounds' },
	soft: { range: [70, 50], description: 'Borders, muted UI' },
	solid: { range: [40, 20], description: 'Buttons, accents' },
	text: { range: [10, 0], description: 'Text, icons' }
} as const;

// Perceptual adjustment constants
const HUE_PERCEPTION = {
	YELLOW_GREEN_PEAK: 120, // Hue perceived as brightest
	BLUE_PURPLE_TROUGH: 240, // Hue perceived as darkest
	MAX_ADJUSTMENT: 0.04 // Maximum lightness boost (4%)
} as const;

const SHADE_THRESHOLDS = {
	BACKGROUND_START: 80,
	SOFT_START: 50,
	SOLID_START: 20,
	DARKEST_FOR_HUE_ADJUSTMENT: 30
} as const;

const LIGHTNESS_RANGES = {
	background: {
		neutral: { min: 0.91, max: 1.0, scale: 0.5 },
		colored: { min: 0.88, max: 0.98, scale: 0.45 }
	},
	soft: { min: 0.43, max: 0.91, exponent: 0.7 },
	solid: { min: 0.22, max: 0.48 },
	text: {
		neutral: { min: 0.08, max: 0.22, exponent: 0.85 },
		colored: { min: 0.11, max: 0.26, exponent: 0.85 }
	}
} as const;

const SATURATION = {
	DESATURATION_START: 60,
	MIN_DESATURATION: 0.1, // 10% at shade 60
	MAX_DESATURATION: 0.15 // 15% at shade 100
} as const;

const NEUTRAL_SCALE_ID = 'ntrl';

// Zone boundaries for calculations
const ZONE_BOUNDARIES = {
	background: { start: 0.8, end: 1.0, width: 0.2 },
	soft: { start: 0.5, end: 0.8, width: 0.3 },
	solid: { start: 0.2, end: 0.5, width: 0.3 },
	text: { start: 0.0, end: 0.2, width: 0.2 }
} as const;

/**
 * Calculate perceptual brightness adjustment based on hue
 * Different hues are perceived as having different brightness even at the same lightness
 * This adjusts lightness to compensate for human color perception
 * Green (120°) is perceived as brightest, Blue/Purple (240°) as darkest
 */
function getHueLightnessAdjustment(hue: number, shade: number): number {
	// Only apply adjustment to darker shades where perception difference is most noticeable
	if (shade > SHADE_THRESHOLDS.DARKEST_FOR_HUE_ADJUSTMENT) return 0;

	const normalizedHue = normalizeHue(hue);
	const perceptualFactor = calculateHuePerceptualFactor(normalizedHue);
	const darknessIntensity = calculateDarknessIntensity(shade);

	return perceptualFactor * HUE_PERCEPTION.MAX_ADJUSTMENT * darknessIntensity;
}

/** Normalize hue to 0-360 range */
function normalizeHue(hue: number): number {
	const normalized = hue % 360;
	return normalized < 0 ? normalized + 360 : normalized;
}

/** Calculate perceptual brightness factor using sine wave (peak at yellow-green, trough at blue-purple) */
function calculateHuePerceptualFactor(normalizedHue: number): number {
	return Math.sin(((normalizedHue - HUE_PERCEPTION.BLUE_PURPLE_TROUGH) * Math.PI) / 180);
}

/** Calculate intensity factor based on shade darkness (darker shades need more correction) */
function calculateDarknessIntensity(shade: number): number {
	const maxDarkness = 50;
	const adjustmentRange = 30;
	return (maxDarkness - shade) / adjustmentRange;
}

/**
 * Perceptual easing function for lightness
 * Uses a power curve to match human perception
 * - Upper range (background): Linear to preserve subtle distinctions
 * - Middle range (soft/solid): Exponential for smoother transitions
 * - Lower range (text): Compressed for better contrast
 *
 * @param shade - The shade value (0-100)
 * @param shade50Target - Optional custom lightness for shade 50 (0-1), which scales adjacent shades
 * @param isNeutral - Whether this is the neutral color scale (uses original lightness values)
 */
function calculateLightness(
	shade: number,
	shade50Target?: number,
	isNeutral: boolean = false
): number {
	const normalizedShade = shade / 100;

	if (shade >= SHADE_THRESHOLDS.BACKGROUND_START) {
		return calculateBackgroundLightness(normalizedShade, isNeutral);
	}

	if (shade >= SHADE_THRESHOLDS.SOFT_START) {
		return calculateSoftLightness(normalizedShade, shade, shade50Target);
	}

	if (shade >= SHADE_THRESHOLDS.SOLID_START) {
		return calculateSolidLightness(normalizedShade, shade, shade50Target);
	}

	return calculateTextLightness(normalizedShade, isNeutral);
}

function calculateBackgroundLightness(normalizedShade: number, isNeutral: boolean): number {
	const range = isNeutral
		? LIGHTNESS_RANGES.background.neutral
		: LIGHTNESS_RANGES.background.colored;

	const zonePosition = normalizedShade - ZONE_BOUNDARIES.background.start;
	return range.min + zonePosition * range.scale;
}

function calculateSoftLightness(
	normalizedShade: number,
	shade: number,
	shade50Target?: number
): number {
	if (shade50Target !== undefined && shade === SHADE_THRESHOLDS.SOFT_START) {
		return shade50Target;
	}

	const zonePosition =
		(normalizedShade - ZONE_BOUNDARIES.soft.start) / ZONE_BOUNDARIES.soft.width;
	const baseLightness =
		LIGHTNESS_RANGES.soft.min +
		Math.pow(zonePosition, LIGHTNESS_RANGES.soft.exponent) *
			(LIGHTNESS_RANGES.soft.max - LIGHTNESS_RANGES.soft.min);

	return applyShade50Adjustment(
		baseLightness,
		shade,
		shade50Target,
		LIGHTNESS_RANGES.soft.min,
		ZONE_BOUNDARIES.soft.width * 100
	);
}

function calculateSolidLightness(
	normalizedShade: number,
	shade: number,
	shade50Target?: number
): number {
	if (shade50Target !== undefined && shade === SHADE_THRESHOLDS.SOFT_START) {
		return shade50Target;
	}

	const zonePosition =
		(normalizedShade - ZONE_BOUNDARIES.solid.start) / ZONE_BOUNDARIES.solid.width;
	const baseLightness =
		LIGHTNESS_RANGES.solid.min +
		zonePosition * (LIGHTNESS_RANGES.solid.max - LIGHTNESS_RANGES.solid.min);

	return applyShade50Adjustment(
		baseLightness,
		shade,
		shade50Target,
		LIGHTNESS_RANGES.solid.max,
		ZONE_BOUNDARIES.solid.width * 100
	);
}

function calculateTextLightness(normalizedShade: number, isNeutral: boolean): number {
	const range = isNeutral
		? LIGHTNESS_RANGES.text.neutral
		: LIGHTNESS_RANGES.text.colored;

	const zonePosition = normalizedShade / ZONE_BOUNDARIES.text.width;
	return range.min + Math.pow(zonePosition, range.exponent) * (range.max - range.min);
}

function applyShade50Adjustment(
	baseLightness: number,
	shade: number,
	shade50Target: number | undefined,
	shade50Base: number,
	zoneWidth: number
): number {
	if (shade50Target === undefined) {
		return baseLightness;
	}

	const adjustment = shade50Target - shade50Base;
	const distanceFromShade50 = Math.abs(shade - SHADE_THRESHOLDS.SOFT_START) / zoneWidth;
	const falloffFactor = Math.pow(1.4 - distanceFromShade50, 1.5);

	return baseLightness + adjustment * falloffFactor;
}

// Generate the lightness map using the mathematical function
export const lightnessMap: Record<number, number> = {
	100: calculateLightness(100),
	95: calculateLightness(95),
	90: calculateLightness(90),
	80: calculateLightness(80),
	70: calculateLightness(70),
	60: calculateLightness(60),
	50: calculateLightness(50),
	40: calculateLightness(40),
	30: calculateLightness(30),
	20: calculateLightness(20),
	10: calculateLightness(10),
	5: calculateLightness(5),
	0: calculateLightness(0)
};

/**
 * Get the semantic zone for a given shade value
 */
export function getSemanticZone(shade: number): keyof typeof ZONES {
	if (shade >= SHADE_THRESHOLDS.BACKGROUND_START) return 'background';
	if (shade >= SHADE_THRESHOLDS.SOFT_START) return 'soft';
	if (shade >= SHADE_THRESHOLDS.SOLID_START) return 'solid';
	return 'text';
}

/**
 * Calculate adjusted saturation for lighter shades to create softer backgrounds
 * Linear reduction: 15% at shade 100, 10% at shade 60, no reduction below 60
 */
function calculateAdjustedSaturation(baseSaturation: number, shadeValue: number): number {
	if (shadeValue < SATURATION.DESATURATION_START) {
		return baseSaturation;
	}

	const rangeAboveThreshold = 100 - SATURATION.DESATURATION_START;
	const shadeDistanceFromThreshold = shadeValue - SATURATION.DESATURATION_START;
	const desaturationFactor =
		SATURATION.MAX_DESATURATION -
		(shadeDistanceFromThreshold / rangeAboveThreshold) *
			(SATURATION.MAX_DESATURATION - SATURATION.MIN_DESATURATION);

	return baseSaturation * (1 - desaturationFactor);
}

export function generateScale(
	scales: ColorScale[],
	shades: Shade[],
	scaleSaturations: Record<string, number>,
	scaleHues: Record<string, number | null>,
	baseHue: number,
	scaleShade50Lightness: Record<string, number>
): Record<string, Record<number, string>> {
	const result: Record<string, Record<number, string>> = {};

	for (const scale of scales) {
		const scaleHue = scaleHues[scale.id] ?? scale.baseHue ?? baseHue;
		const baseSaturation = scaleSaturations[scale.id] / 100;
		const shade50Lightness = scaleShade50Lightness[scale.id] / 100;
		const isNeutral = scale.id === NEUTRAL_SCALE_ID;

		result[scale.id] = generateScaleShades(
			scaleHue,
			baseSaturation,
			shade50Lightness,
			isNeutral,
			shades
		);
	}

	return result;
}

function generateScaleShades(
	hue: number,
	baseSaturation: number,
	shade50Lightness: number,
	isNeutral: boolean,
	shades: Shade[]
): Record<number, string> {
	const scaleShades: Record<number, string> = {};

	for (const shade of shades) {
		let lightness = calculateLightness(shade.value, shade50Lightness, isNeutral);

		// Apply perceptual brightness adjustment based on hue
		if (!isNeutral) {
			lightness += getHueLightnessAdjustment(hue, shade.value);
		}

		// Reduce saturation for lighter shades to create softer backgrounds
		const saturation = calculateAdjustedSaturation(baseSaturation, shade.value);

		const rgb = hslToRgb(hue, saturation, lightness);
		scaleShades[shade.value] = rgbToHex(rgb.r, rgb.g, rgb.b);
	}

	return scaleShades;
}
