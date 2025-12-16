/**
 * Color scale constants and configuration
 */

import type { ColorScale, Shade, SemanticZone } from '../types/color';

export const SCALES: ColorScale[] = [
	{ name: 'Gray', id: 'gray', baseHue: 25 },
	{ name: 'Pop (Teal)', id: 'pop', baseHue: 180 },
	{ name: 'Error (Red)', id: 'err', baseHue: 8 },
	{ name: 'Warning (Orange)', id: 'warn', baseHue: 35 },
	{ name: 'Success (Green)', id: 'succ', baseHue: 155 },
	{ name: 'Purple', id: 'purp', baseHue: 270 }
];

export const SHADES: Shade[] = [
	{ value: 100, purpose: 'background' },
	{ value: 95, purpose: 'background' },
	{ value: 90, purpose: 'background' },
	{ value: 80, purpose: 'soft' },
	{ value: 70, purpose: 'soft' },
	{ value: 60, purpose: 'soft' },
	{ value: 50, purpose: 'solid' },
	{ value: 40, purpose: 'solid' },
	{ value: 30, purpose: 'text' },
	{ value: 20, purpose: 'text' },
	{ value: 10, purpose: 'text' },
	{ value: 5, purpose: 'text' },
	{ value: 0, purpose: 'text' }
];

export const SEMANTIC_ZONES: SemanticZone[] = [
	{ label: 'Background', range: '100-90' },
	{ label: 'Soft', range: '80-60' },
	{ label: 'Solid', range: '50-40' },
	{ label: 'Text', range: '30-0' }
];

export const DEFAULT_SATURATIONS: Record<string, number> = {
	gray: 8,
	pop: 65,
	succ: 60,
	err: 68,
	warn: 95,
	purp: 60
};

export const DEFAULT_SHADE_50_LIGHTNESS: Record<string, number> = {
	gray: 45,
	pop: 42,
	err: 50,
	warn: 50,
	succ: 45,
	purp: 54
};
