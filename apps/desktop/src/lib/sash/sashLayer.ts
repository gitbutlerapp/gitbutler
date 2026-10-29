export const SASH_LAYER = Symbol("SashLayer");

export interface SashLayerContext {
	container: HTMLElement | undefined;
	requestLayout: () => void;
	subscribeLayout: (listener: (containerRect: DOMRectReadOnly) => void) => () => void;
	observeLayoutTarget: (target: Element) => () => void;
	setAutoLayoutPaused: (paused: boolean) => void;
}
