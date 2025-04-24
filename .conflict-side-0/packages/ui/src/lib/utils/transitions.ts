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
	const nodeStyle = getComputedStyle(node);
	const transformX = new WebKitCSSMatrix(nodeStyle.transform).m41;

	// Default values
	const DEFAULT_Y = -6;
	const DEFAULT_SCALE_START = 0.94;
	const DEFAULT_DURATION = 200;
	const DEFAULT_POSITION = 'top';

	// Extracting and using default values
	const y = params.y ?? DEFAULT_Y;
	const startScale = params.start ?? DEFAULT_SCALE_START;
	const duration = params.duration ?? DEFAULT_DURATION;
	const position = params.position ?? DEFAULT_POSITION;

	return {
		duration,
		css: (t) => {
			const translateY = y * (1 - t);
			const scale = startScale + t * (1 - startScale);

			return `transform: translate3d(${transformX}px, ${pxToRem(position === 'top' ? -translateY : translateY)}, 0) scale(${scale});
			        opacity: ${t};`;
		},
		easing: cubicOut
	};
}
