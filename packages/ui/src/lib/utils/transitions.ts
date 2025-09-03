import { pxToRem } from '$lib/utils/pxToRem';
import { cubicOut } from 'svelte/easing';
import { slide, type SlideParams, type TransitionConfig } from 'svelte/transition';

export function slideFade(node: Element, options: SlideParams): TransitionConfig {
	const slideTrans: TransitionConfig = slide(node, options);

	return {
		...slideTrans,
		css: (t, u) =>
			`${slideTrans.css ? slideTrans.css(t, u) : ''}
			opacity:${t};`
	};
}

// Constants moved outside function for better performance
const DEFAULT_Y = -6;
const DEFAULT_SCALE_START = 0.94;
const DEFAULT_DURATION = 200;
const DEFAULT_POSITION = 'top';

export function flyScale(
	node: Element,
	params: {
		y?: number;
		x?: number;
		start?: number;
		duration?: number;
		position?: 'top' | 'bottom';
	} = {}
): TransitionConfig {
	// Pre-calculate static values
	const nodeStyle = getComputedStyle(node);
	const transformX = new WebKitCSSMatrix(nodeStyle.transform).m41;

	// Use destructuring with defaults for cleaner code
	const {
		y = DEFAULT_Y,
		start: startScale = DEFAULT_SCALE_START,
		duration = DEFAULT_DURATION,
		position = DEFAULT_POSITION
	} = params;

	// Pre-calculate position multiplier to avoid repeated conditional checks
	const positionMultiplier = position === 'top' ? -1 : 1;
	const scaleRange = 1 - startScale;

	return {
		duration,
		css: (t) => {
			const translateY = y * (1 - t);
			const scale = startScale + t * scaleRange;
			const translateYRem = pxToRem(positionMultiplier * translateY);

			return `transform: translate3d(${transformX}px, ${translateYRem}rem, 0) scale(${scale}); opacity: ${t};`;
		},
		easing: cubicOut
	};
}

export function popIn(
	node: Element,
	{
		delay = 100,
		duration = 200,
		transformOrigin = 'left bottom'
	}: {
		delay?: number;
		duration?: number;
		transformOrigin?: string;
	} = {}
) {
	return {
		delay,
		duration,
		easing: cubicOut,
		css: (t: number) => {
			const scale = 0.2 + 0.8 * t;
			const y = 15 * (1 - t);
			const rotate = -8 * (1 - t);
			return `
					transform-origin: ${transformOrigin};
					transform: scale(${scale}) translateY(${y}px) rotate(${rotate}deg);
					opacity: ${t};
				`;
		}
	};
}
