<script lang="ts">
	import { RESIZE_SYNC } from "$lib/floating/resizeSync";
	import { SASH_LAYER } from "$lib/sash/sashLayer";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { persistWithExpiration } from "@gitbutler/shared/persisted";
	import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";
	import { pxToRem, remToPx } from "@gitbutler/ui/utils/pxToRem";
	import { getContext, onDestroy } from "svelte";
	import { on } from "svelte/events";
	import { writable } from "svelte/store";
	import type { ResizeGroup } from "$lib/floating/resizeGroup";
	import type { SashLayerContext } from "$lib/sash/sashLayer";

	interface Props {
		/** Default value */
		defaultValue: number | undefined;
		/** The element that is being resized */
		viewport: HTMLElement;
		/** Sets direction of resizing for viewport */
		direction: "left" | "right" | "up" | "down";
		/** Custom z-index in case of overlapping with other elements */
		zIndex?: string;
		/** Other resizers with the same name will receive same updates. */
		syncName?: string;
		/** Name under which the latest width is stored. */
		persistId?: string;
		/** Minimum width for the resizable element */
		minWidth?: number;
		maxWidth?: number;
		maxHeight?: number;
		minHeight?: number;
		/** Enabled, but does not set the width/height on the dom element */
		passive?: boolean;
		/** Whether the resizer is disabled */
		disabled?: boolean;
		/** Optional manager that can coordinate multiple resizers */
		resizeGroup?: ResizeGroup;
		/** Optional ordering of resizer for use with `resizeManager` */
		order?: number;
		/** Unset max height */
		unsetMaxHeight?: string;
		/** Optional visual offset from the viewport edge used to place the sash */
		edgeOffsetRem?: number;
		/** In layer mode, stretch sash across full layer cross-axis */
		fullLayerCrossAxis?: boolean;

		// Actions
		onWidth?: (width: number) => void;
		onResizing?: (isResizing: boolean) => void;
		onOverflow?: (value: number) => void;
		onDblClick?: () => void;
	}

	let {
		defaultValue,
		viewport,
		direction,
		zIndex = "var(--z-lifted)",
		minWidth = 0,
		maxWidth = 120,
		minHeight = 0,
		maxHeight = 120,
		syncName,
		persistId,
		passive,
		disabled,
		resizeGroup,
		order,
		unsetMaxHeight,
		edgeOffsetRem = 0,
		fullLayerCrossAxis = false,
		onResizing,
		onOverflow,
		onDblClick,
		onWidth,
	}: Props = $props();

	const orientation = $derived(["left", "right"].includes(direction) ? "horizontal" : "vertical");
	const uiState = inject(UI_STATE);
	const resizeSync = inject(RESIZE_SYNC);
	const zoom = $derived(uiState.global.zoom.current);

	const value = $derived(
		persistId
			? persistWithExpiration(defaultValue, persistId, 1440)
			: writable<number | undefined>(defaultValue),
	);

	const resizerId = Symbol();

	// Resizer requires a SashLayer ancestor and always renders in that overlay.
	// Keep SashLayer in the same scroll context as the viewport so
	// getBoundingClientRect differences remain scroll-invariant.
	const layerCtxMaybe = getContext<SashLayerContext | undefined>(SASH_LAYER);
	if (!layerCtxMaybe) {
		throw new Error("Resizer must be used inside <SashLayer>.");
	}
	const layerCtx = layerCtxMaybe;

	let initial = 0;
	let isResizing = $state(false);
	let resizerDiv = $state<HTMLDivElement>();
	let pointerMoveRaf: number | undefined;
	let pendingPointerMove:
		| {
				clientX: number;
				clientY: number;
				shiftKey: boolean;
		  }
		| undefined;

	let unsubUp: (() => void) | undefined;
	let unsubMove: (() => void) | undefined;

	// Last pointer position tracked per-drag-frame for arithmetic sash movement.
	let lastDragClientX = 0;
	let lastDragClientY = 0;
	let lastDragSashPosPx = 0;
	let lastShiftSyncedValue: number | undefined;
	let lastReportedWidth: number | undefined;
	let pausedAutoLayout = false;

	type StyleProp =
		| "left"
		| "right"
		| "top"
		| "bottom"
		| "width"
		| "height"
		| "maxWidth"
		| "minWidth"
		| "maxHeight"
		| "minHeight"
		| "flexBasis"
		| "flexGrow"
		| "flexShrink";

	function setStyle(style: CSSStyleDeclaration, prop: StyleProp, value: string) {
		if (style[prop] !== value) {
			style[prop] = value;
		}
	}

	function reportWidth(nextValue: number | undefined) {
		if (nextValue === undefined) {
			lastReportedWidth = undefined;
			return;
		}
		if (nextValue === lastReportedWidth) {
			return;
		}
		lastReportedWidth = nextValue;
		onWidth?.(nextValue);
	}

	function cleanupPointerDragState() {
		unsubUp?.();
		unsubUp = undefined;
		unsubMove?.();
		unsubMove = undefined;

		if (pointerMoveRaf !== undefined) {
			cancelAnimationFrame(pointerMoveRaf);
			pointerMoveRaf = undefined;
		}

		pendingPointerMove = undefined;
		lastShiftSyncedValue = undefined;

		if (pausedAutoLayout) {
			layerCtx.setAutoLayoutPaused(false);
			pausedAutoLayout = false;
		}
	}

	onDestroy(() => {
		cleanupPointerDragState();
	});

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		unsubUp = on(document, "pointerup", onMouseUp);
		unsubMove = on(document, "pointermove", onMouseMove);

		if (direction === "right") initial = e.clientX - viewport.clientWidth;
		if (direction === "left") initial = window.innerWidth - e.clientX - viewport.clientWidth;
		if (direction === "down") initial = e.clientY - viewport.clientHeight;
		if (direction === "up") initial = window.innerHeight - e.clientY - viewport.clientHeight;

		// Capture starting pointer position for drag-delta sash movement.
		lastDragClientX = e.clientX;
		lastDragClientY = e.clientY;
		lastShiftSyncedValue = undefined;
		if (resizerDiv) {
			lastDragSashPosPx =
				parseFloat(orientation === "horizontal" ? resizerDiv.style.left : resizerDiv.style.top) ||
				0;
		}
		if (!resizeGroup && !pausedAutoLayout) {
			layerCtx.setAutoLayoutPaused(true);
			pausedAutoLayout = true;
		}

		onResizing?.(true);
	}

	function applyLimits(nextValue: number) {
		const minValue = orientation === "horizontal" ? minWidth : minHeight;
		const maxValue = orientation === "horizontal" ? maxWidth : maxHeight;
		const newValue = Math.min(Math.max(nextValue, minValue), maxValue);
		return { newValue, overflow: minValue - nextValue };
	}

	function processPointerMove() {
		const move = pendingPointerMove;
		if (!move) {
			return;
		}

		pendingPointerMove = undefined;
		let offsetPx: number | undefined;
		switch (direction) {
			case "down":
				offsetPx = move.clientY - initial;
				break;
			case "up":
				offsetPx = document.body.scrollHeight - move.clientY - initial;
				break;
			case "right":
				offsetPx = move.clientX - initial;
				break;
			case "left":
				offsetPx = document.body.scrollWidth - move.clientX - initial;
				break;
		}

		const offsetRem = pxToRem(offsetPx, zoom);

		// Presence of a resize group means we hand off the rest of the
		// handling of this event.
		if (resizeGroup) {
			const subtracted = resizeGroup.resize(resizerId, offsetRem);
			// The initial offset needs to be adjusted if an adjustment
			// means the whole resizer has moved.
			initial = initial - remToPx(subtracted, zoom);
			return;
		}

		const { newValue, overflow } = applyLimits(offsetRem);

		if (newValue !== undefined && !passive && !disabled) {
			// Fast path for direct drag feedback: move sash by pointer delta
			// (no getBoundingClientRect calls during drag). Geometry is re-synced
			// once on mouse-up via requestLayout.
			if (resizerDiv) {
				const dx = move.clientX - lastDragClientX;
				const dy = move.clientY - lastDragClientY;
				if (orientation === "horizontal") {
					lastDragSashPosPx += dx;
					setStyle(resizerDiv.style, "left", `${lastDragSashPosPx}px`);
				} else {
					lastDragSashPosPx += dy;
					setStyle(resizerDiv.style, "top", `${lastDragSashPosPx}px`);
				}
				// Write viewport size and notify callbacks — no requestLayout here.
				if (newValue !== $value) {
					value.set(newValue);
					updateDom(newValue, true);
					reportWidth(newValue);
				}
			} else {
				setValue(newValue);
			}
		}

		lastDragClientX = move.clientX;
		lastDragClientY = move.clientY;

		if (overflow > 0) {
			onOverflow?.(overflow);
		}
		if (
			move.shiftKey &&
			syncName &&
			newValue !== undefined &&
			!passive &&
			!disabled &&
			newValue !== lastShiftSyncedValue
		) {
			lastShiftSyncedValue = newValue;
			resizeSync.emit(syncName, resizerId, newValue);
		}
	}

	function onMouseMove(e: MouseEvent) {
		isResizing = true;
		pendingPointerMove = {
			clientX: e.clientX,
			clientY: e.clientY,
			shiftKey: e.shiftKey,
		};

		if (pointerMoveRaf !== undefined) {
			return;
		}

		pointerMoveRaf = requestAnimationFrame(() => {
			pointerMoveRaf = undefined;
			processPointerMove();
		});
	}

	function onMouseUp() {
		if (pointerMoveRaf !== undefined) {
			cancelAnimationFrame(pointerMoveRaf);
			pointerMoveRaf = undefined;
		}
		processPointerMove();
		cleanupPointerDragState();
		// Re-sync sash to exact geometry once at the end of drag.
		layerCtx.requestLayout();
		isResizing = false;
		onResizing?.(false);
	}

	function onclick(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
	}

	function updateDom(newValue?: number, valueAlreadyLimited = false) {
		if (!viewport) {
			return;
		}
		if (passive || disabled) {
			if (orientation === "horizontal") {
				setStyle(viewport.style, "width", "");
				setStyle(viewport.style, "flexBasis", "");
				setStyle(viewport.style, "flexGrow", "");
				setStyle(viewport.style, "flexShrink", "");
				setStyle(viewport.style, "maxWidth", "");
				setStyle(viewport.style, "minWidth", "");
			} else {
				setStyle(viewport.style, "height", "");
				setStyle(viewport.style, "maxHeight", "");
				setStyle(viewport.style, "minHeight", "");
			}
			return;
		}

		const limitedValue =
			newValue !== undefined && !valueAlreadyLimited ? applyLimits(newValue).newValue : newValue;

		if (orientation === "horizontal") {
			if (limitedValue === undefined) {
				setStyle(viewport.style, "width", "");
				// Restore flex behaviour so CSS classes take over again.
				setStyle(viewport.style, "flexBasis", "");
				setStyle(viewport.style, "flexGrow", "");
				setStyle(viewport.style, "flexShrink", "");
				setStyle(viewport.style, "maxWidth", maxWidth ? maxWidth + "rem" : "");
				setStyle(viewport.style, "minWidth", minWidth ? minWidth + "rem" : "");
			} else {
				const remValue = limitedValue + "rem";
				setStyle(viewport.style, "width", remValue);
				// Pin flex-basis to the explicit value and lock grow/shrink so
				// the flex algorithm cannot override the user-set width.
				setStyle(viewport.style, "flexBasis", remValue);
				setStyle(viewport.style, "flexGrow", "0");
				setStyle(viewport.style, "flexShrink", "0");
				setStyle(viewport.style, "maxWidth", "");
				setStyle(viewport.style, "minWidth", "");
			}
		} else {
			if (limitedValue === undefined) {
				setStyle(viewport.style, "height", "");
				setStyle(viewport.style, "maxHeight", unsetMaxHeight || "");
				setStyle(viewport.style, "minHeight", minHeight ? minHeight + "rem" : "");
			} else {
				setStyle(viewport.style, "height", limitedValue + "rem");
				setStyle(viewport.style, "maxHeight", "");
				setStyle(viewport.style, "minHeight", "");
			}
		}
	}

	function getValue() {
		if ($value !== undefined) {
			return $value;
		}
		if (orientation === "horizontal") {
			return pxToRem(viewport.clientWidth, zoom);
		}
		return pxToRem(viewport.clientHeight, zoom);
	}

	export function setValue(newValue?: number) {
		if (newValue !== undefined) {
			newValue = applyLimits(newValue).newValue;
		}
		const currentValue = getValue();
		if (currentValue === newValue) {
			return;
		}
		value.set(newValue);
		updateDom(newValue, true);
		reportWidth(newValue);
		layerCtx.requestLayout();
	}

	$effect(() => {
		if (resizeGroup && order !== undefined) {
			// It's important we do not make use of maxValue in the resize
			// manager, and in this effect. It changes with the value of
			// neighbors and would make this effect trigger constantly.
			return resizeGroup.register({
				resizerId,
				getValue,
				setValue,
				minValue: minHeight || minWidth,
				position: order,
			});
		}
	});

	$effect(() => {
		if (syncName) {
			const unlistenFns = [];
			unlistenFns.push(
				resizeSync.subscribe({
					key: syncName,
					resizerId,
					callback: setValue,
				}),
			);
			return mergeUnlisten(...unlistenFns);
		}
	});

	$effect(() => {
		if (maxWidth || minWidth || maxHeight || minHeight) {
			updateDom($value);
			reportWidth($value);
		}
	});

	// Overlay effect: move the resizer div into the SashLayer overlay so it is
	// never clipped by overflow:hidden on pane containers. Position is
	// kept in sync via ResizeObserver + window resize + shared per-layer
	// scheduler notifications. No scroll tracking is needed — the SashLayer is
	// always scoped inside the same scroll container as the viewport, so
	// getBoundingClientRect differences are scroll-invariant.
	$effect(() => {
		const container = layerCtx.container;
		const div = resizerDiv;
		const vp = viewport;
		// Snapshot these so updatePosition closes over stable values. They are
		// read here (inside the effect body) so Svelte tracks them as deps.
		const dir = direction;
		const orient = orientation;
		// Keep the divider center correction separate from API semantics:
		// edgeOffsetRem is caller intent; +0.5 aligns to 1px separator center.
		const effectiveEdgeOffsetPx = remToPx(edgeOffsetRem, zoom) + 0.5;

		if (!container || !div || !vp) return;

		// Re-bind with narrowed types so closures below satisfy TypeScript.
		// (Type narrowing from if-guards doesn't propagate into nested functions.)
		const c = container;
		const d = div;
		const applyMarginCompensation = fullLayerCrossAxis;
		const vpStyle = applyMarginCompensation ? getComputedStyle(vp) : undefined;
		const marginLeftPx = applyMarginCompensation ? parseFloat(vpStyle?.marginLeft || "0") || 0 : 0;
		const marginRightPx = applyMarginCompensation
			? parseFloat(vpStyle?.marginRight || "0") || 0
			: 0;

		c.appendChild(d);

		function updatePosition(containerRect?: DOMRectReadOnly) {
			const vr = vp.getBoundingClientRect();
			const cr = containerRect ?? c.getBoundingClientRect();
			// 6 px hit area in layer mode (no risk of overlapping scrollbars).
			const t = 6;
			const crossTopPx = fullLayerCrossAxis ? 0 : vr.top - cr.top;
			const crossHeightPx = fullLayerCrossAxis ? cr.height : vr.height;

			if (orient === "horizontal") {
				const edge =
					dir === "right"
						? vr.right + marginRightPx + effectiveEdgeOffsetPx
						: vr.left - marginLeftPx - effectiveEdgeOffsetPx;
				setStyle(d.style, "left", `${edge - cr.left - t / 2}px`);
				setStyle(d.style, "right", "");
				setStyle(d.style, "top", `${crossTopPx}px`);
				setStyle(d.style, "bottom", "");
				setStyle(d.style, "width", `${t}px`);
				setStyle(d.style, "height", `${crossHeightPx}px`);
			} else {
				const edge =
					dir === "down" ? vr.bottom + effectiveEdgeOffsetPx : vr.top - effectiveEdgeOffsetPx;
				setStyle(d.style, "top", `${edge - cr.top - t / 2}px`);
				setStyle(d.style, "bottom", "");
				setStyle(d.style, "left", `${vr.left - cr.left}px`);
				setStyle(d.style, "right", "");
				setStyle(d.style, "width", `${vr.width}px`);
				setStyle(d.style, "height", `${t}px`);
			}
		}

		updatePosition();

		const unobserveLayoutTarget = layerCtx.observeLayoutTarget(vp);
		const unsubscribeLayout = layerCtx.subscribeLayout((containerRect) => {
			updatePosition(containerRect);
		});
		layerCtx.requestLayout();

		return () => {
			if (pausedAutoLayout) {
				layerCtx.setAutoLayoutPaused(false);
				pausedAutoLayout = false;
			}
			unobserveLayoutTarget?.();
			unsubscribeLayout?.();
			d.remove();
		};
	});
</script>

<div
	role="presentation"
	bind:this={resizerDiv}
	data-no-drag
	onpointerdown={onMouseDown}
	ondblclick={() => {
		onDblClick?.();
		setValue(defaultValue);
	}}
	{onclick}
	class:disabled
	class="resizer"
	class:is-resizing={isResizing}
	class:vertical={orientation === "vertical"}
	class:horizontal={orientation === "horizontal"}
	class:up={direction === "up"}
	class:down={direction === "down"}
	class:left={direction === "left"}
	class:right={direction === "right"}
	style:z-index={zIndex}
></div>

<style lang="postcss">
	.resizer {
		--resizer-thickness: 4px;
		--resizer-cursor: default;
		position: absolute;
		outline: none;
		cursor: var(--resizer-cursor);
		/* background-color: rgba(255, 0, 0, 0.345); */
		pointer-events: initial;

		&.horizontal {
			--resizer-cursor: col-resize;
			top: 0;
			width: var(--resizer-thickness);
			height: 100%;
		}

		&.vertical {
			--resizer-cursor: row-resize;
			left: 0;
			width: 100%;
			height: var(--resizer-thickness);
		}

		/* Anchor-to-edge defaults before the first layout pass. */
		&.horizontal.right {
			right: 0;
			left: auto;
		}

		&.horizontal.left {
			right: auto;
			left: 0;
		}

		&.vertical.down {
			top: auto;
			bottom: 0;
		}

		&.vertical.up {
			top: 0;
			bottom: auto;
		}

		&.disabled {
			pointer-events: none;
			--resizer-cursor: default;
		}
	}
</style>
