/**
 * WCAG contrast ratio calculations
 */

import { hexToRgb } from './colorConversion';

export function getRelativeLuminance(r: number, g: number, b: number): number {
	const [rs, gs, bs] = [r, g, b].map((c) => {
		return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
	});
	return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
}

export type ContrastLevel = 'AAA' | 'AA' | 'AA Large' | 'Fail';

export interface ContrastResult {
	ratio: number;
	level: ContrastLevel;
	passes: {
		aaaLarge: boolean;
		aaa: boolean;
		aaLarge: boolean;
		aa: boolean;
	};
}

export function getContrastRatio(hex1: string, hex2: string): ContrastResult {
	const rgb1 = hexToRgb(hex1);
	const rgb2 = hexToRgb(hex2);

	if (!rgb1 || !rgb2) {
		return {
			ratio: 0,
			level: 'Fail',
			passes: {
				aaaLarge: false,
				aaa: false,
				aaLarge: false,
				aa: false
			}
		};
	}

	const l1 = getRelativeLuminance(rgb1.r, rgb1.g, rgb1.b);
	const l2 = getRelativeLuminance(rgb2.r, rgb2.g, rgb2.b);

	const lighter = Math.max(l1, l2);
	const darker = Math.min(l1, l2);

	const ratio = (lighter + 0.05) / (darker + 0.05);

	return {
		ratio: Math.round(ratio * 100) / 100,
		level: ratio >= 7 ? 'AAA' : ratio >= 4.5 ? 'AA' : ratio >= 3 ? 'AA Large' : 'Fail',
		passes: {
			aaaLarge: ratio >= 4.5,
			aaa: ratio >= 7,
			aaLarge: ratio >= 3,
			aa: ratio >= 4.5
		}
	};
}
