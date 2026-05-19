import {
	getMotionDelay,
	getMotionDistance,
	getMotionDuration,
	motionDurations,
	prefersReducedMotion,
} from "$lib/utils/motion";
import { pxToRem } from "$lib/utils/pxToRem";
import { cubicOut } from "svelte/easing";
import { slide, type SlideParams, type TransitionConfig } from "svelte/transition";

export function slideFade(node: Element, options: SlideParams = {}): TransitionConfig {
	const slideTrans: TransitionConfig = slide(node, {
		...options,
		duration: getMotionDuration(options.duration ?? motionDurations.medium),
	});

	return {
		...slideTrans,
		css: (t, u) =>
			`${slideTrans.css ? slideTrans.css(t, u) : ""}
			opacity:${t};`,
	};
}

// Constants moved outside function for better performance
const DEFAULT_OFFSET = 6;
const DEFAULT_SCALE_START = 0.94;
const DEFAULT_DURATION = motionDurations.overlay;
const DEFAULT_POSITION = "top";

export function flyScale(
	node: Element,
	params: {
		y?: number;
		x?: number;
		start?: number;
		duration?: number;
		position?: "top" | "bottom" | "left" | "right";
	} = {},
): TransitionConfig {
	// Pre-calculate static values
	const nodeStyle = getComputedStyle(node);
	const transformX = new WebKitCSSMatrix(nodeStyle.transform).m41;
	const reducedMotion = prefersReducedMotion();

	// Use destructuring with defaults for cleaner code
	const {
		y = DEFAULT_OFFSET,
		x = DEFAULT_OFFSET,
		start: startScale = DEFAULT_SCALE_START,
		duration = DEFAULT_DURATION,
		position = DEFAULT_POSITION,
	} = params;

	const motionX = getMotionDistance(x);
	const motionY = getMotionDistance(y);
	const initialScale = reducedMotion ? 1 : startScale;
	const scaleRange = 1 - initialScale;

	return {
		duration: getMotionDuration(duration),
		css: (t) => {
			const translateX =
				position === "left" ? motionX * (1 - t) : position === "right" ? -motionX * (1 - t) : 0;
			const translateY =
				position === "top" ? motionY * (1 - t) : position === "bottom" ? -motionY * (1 - t) : 0;
			const scale = initialScale + t * scaleRange;
			const translateYRem = pxToRem(translateY);

			return `transform: translate3d(${transformX + translateX}px, ${translateYRem}rem, 0) scale(${scale}); opacity: ${t};`;
		},
		easing: cubicOut,
	};
}

export function popIn(
	node: Element,
	{
		delay = 100,
		duration = motionDurations.overlay,
		transformOrigin = "left bottom",
	}: {
		delay?: number;
		duration?: number;
		transformOrigin?: string;
	} = {},
) {
	const reducedMotion = prefersReducedMotion();
	const startScale = reducedMotion ? 1 : 0.2;
	const translateYDistance = getMotionDistance(15);
	const rotateDistance = reducedMotion ? 0 : -8;

	return {
		delay: getMotionDelay(delay),
		duration: getMotionDuration(duration),
		easing: cubicOut,
		css: (t: number) => {
			const scale = startScale + (1 - startScale) * t;
			const y = translateYDistance * (1 - t);
			const rotate = rotateDistance * (1 - t);
			return `
					transform-origin: ${transformOrigin};
					transform: scale(${scale}) translateY(${y}px) rotate(${rotate}deg);
					opacity: ${t};
				`;
		},
	};
}
