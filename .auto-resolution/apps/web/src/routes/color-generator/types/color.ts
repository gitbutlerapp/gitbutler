/**
 * Type definitions for color scales
 */

export interface ColorScale {
	name: string;
	id: string;
	baseHue: number;
}

export interface Shade {
	value: number;
	purpose: 'background' | 'soft' | 'solid' | 'text';
}

export interface SemanticZone {
	label: string;
	range: string;
}
