import { pxToRem } from '$lib/utils/pxToRem';
import { sineInOut } from 'svelte/easing';
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

// extend SlideParams with opacity
type SlideFadeParams = SlideParams & {
	opacity?: number;
	minHeight?: number;
	animateTopPadding?: boolean;
	animateBottomPadding?: boolean;
	animateLeftPadding?: boolean;
	animateRightPadding?: boolean;
};

export function slideFadeExt(node: Element, options: SlideFadeParams): TransitionConfig {
	const slideTrans: TransitionConfig = slide(node, options);

	const currentHeight = node.clientHeight;
	const minHeight = options.minHeight || 0;

	const currentPadding = {
		top: pxToRem(+getComputedStyle(node).paddingTop),
		bottom: pxToRem(+getComputedStyle(node).paddingBottom),
		left: pxToRem(+getComputedStyle(node).paddingLeft),
		right: pxToRem(+getComputedStyle(node).paddingRight)
	};

	return {
		...slideTrans,
		easing: sineInOut,
		css: (t) =>
			`height: ${minHeight + (currentHeight - minHeight) * Math.min(Math.max(t, 0), 1)}px; 
			opacity:${t};
			padding-top: ${options.animateTopPadding ? 0 : currentPadding.top};
			padding-bottom: ${options.animateBottomPadding ? 0 : currentPadding.bottom};
			padding-left: ${options.animateLeftPadding ? 0 : currentPadding.left};
			padding-right: ${options.animateRightPadding ? 0 : currentPadding.right};
			`
	};
}
