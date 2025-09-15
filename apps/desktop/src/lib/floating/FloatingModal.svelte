<script lang="ts">
	import ResizeHandles from '$lib/floating/ResizeHandles.svelte';
	import { DragResizeHandler } from '$lib/floating/dragResizeHandler';
	import { ResizeCalculator } from '$lib/floating/resizeCalculator';
	import { SnapPointManager } from '$lib/floating/snapPointManager';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { onMount, type Snippet } from 'svelte';
	import type { SnapPositionName } from '$lib/floating/types';
	import type { SnapPoint, ModalBounds } from '$lib/floating/types';

	interface Props {
		children: Snippet;
		dragHandleElement?: HTMLElement;
		defaults: {
			snapPosition: string;
			width: number;
			minWidth: number;
			height: number;
			minHeight: number;
		};
		onUpdateSnapPosition?: (snapPosition: SnapPositionName) => void;
		onUpdateSize?: (width: number, height: number) => void;
		onCancel?: () => void;
	}

	const {
		children,
		dragHandleElement,
		defaults,
		onUpdateSnapPosition,
		onUpdateSize,
		onCancel
	}: Props = $props();

	// Managers
	const snapManager = new SnapPointManager(40);
	const resizeCalculator = new ResizeCalculator(defaults.minWidth, defaults.minHeight);
	const dragResizeHandler = new DragResizeHandler(snapManager, resizeCalculator);

	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	// Modal state
	let x = $state(0);
	let y = $state(0);
	let width = $state(defaults.width);
	let height = $state(defaults.height);
	let currentSnapPoint: SnapPoint | null = $state(null);
	let snapping = $state(false);
	let snapPoints: SnapPoint[] = [];

	// DOM reference
	let modalEl: HTMLDivElement;

	// Animation helper
	function animateToPosition(newX: number, newY: number, threshold = 5) {
		if (Math.abs(x - newX) > threshold || Math.abs(y - newY) > threshold) {
			snapping = true;
			setTimeout(() => {
				snapping = false;
			}, 300);
		}
		x = newX;
		y = newY;
	}

	// Snap to nearest point
	function snapToNearestPoint() {
		const modalCenterX = x + width / 2;
		const modalCenterY = y + height / 2;
		const nearestSnapPoint = snapManager.findNearestSnapPoint(
			modalCenterX,
			modalCenterY,
			snapPoints
		);

		const { offsetX, offsetY } = snapManager.getAlignmentOffset(
			nearestSnapPoint.x,
			nearestSnapPoint.y,
			width,
			height
		);

		const newX = nearestSnapPoint.x + offsetX;
		const newY = nearestSnapPoint.y + offsetY;

		animateToPosition(newX, newY);

		currentSnapPoint = nearestSnapPoint;

		onUpdateSnapPosition?.(nearestSnapPoint.name);
	}

	// Update position maintaining snap point
	function updatePositionForSnapPoint() {
		if (!currentSnapPoint) return;

		const updatedSnapPoint = snapPoints.find((p) => p.name === currentSnapPoint!.name);
		if (!updatedSnapPoint) return;

		currentSnapPoint = updatedSnapPoint;
		const { offsetX, offsetY } = snapManager.getAlignmentOffset(
			currentSnapPoint.x,
			currentSnapPoint.y,
			width,
			height
		);

		x = currentSnapPoint.x + offsetX;
		y = currentSnapPoint.y + offsetY;
	}

	// Event handlers
	function handleHeaderPointerDown(event: PointerEvent) {
		event.stopPropagation();
		dragResizeHandler.startDrag(event, { x, y, width, height });
	}

	function handleResizeStart(event: PointerEvent, direction: string) {
		event.stopPropagation();
		dragResizeHandler.startResize(event, direction, { x, y, width, height });
	}

	function handleWindowResize() {
		snapPoints = snapManager.calcSnapPoints();
		updatePositionForSnapPoint();

		// Constrain size to viewport
		const maxWidth = window.innerWidth - 80; // Leave 40px margin on each side
		const maxHeight = window.innerHeight - 80; // Leave 40px margin on top and bottom
		width = Math.min(width, maxWidth);
		height = Math.min(height, maxHeight);

		// Constrain position to viewport
		const constrainedPosition = snapManager.constrainToViewport({ x, y, width, height });
		x = Math.max(40, Math.min(x, constrainedPosition.x));
		y = Math.max(40, Math.min(y, constrainedPosition.y));
	}

	// Setup drag/resize callbacks
	dragResizeHandler.onDrag = (bounds: ModalBounds) => {
		x = bounds.x;
		y = bounds.y;
	};

	dragResizeHandler.onResize = (bounds: ModalBounds) => {
		// Constrain size to viewport
		const maxWidth = window.innerWidth - 80; // Leave 40px margin on each side
		const maxHeight = window.innerHeight - 80; // Leave 40px margin on top and bottom

		x = bounds.x;
		y = bounds.y;
		width = Math.min(bounds.width, maxWidth);
		height = Math.min(bounds.height, maxHeight);

		onUpdateSize?.(width, height);
	};

	dragResizeHandler.onDragEnd = () => {
		snapToNearestPoint();
	};

	dragResizeHandler.onResizeEnd = () => {
		const constrainedPosition = snapManager.constrainToViewport({ x, y, width, height });
		if (Math.abs(x - constrainedPosition.x) > 1 || Math.abs(y - constrainedPosition.y) > 1) {
			animateToPosition(constrainedPosition.x, constrainedPosition.y, 1);
		}
		// Don't snap to nearest point on resize end - maintain current position
	};

	// Update current snap position for resize calculations
	$effect(() => {
		dragResizeHandler.currentSnapPosition = currentSnapPoint?.name || '';
	});

	onMount(() => {
		snapPoints = snapManager.calcSnapPoints();

		// Constrain initial size to viewport
		const maxWidth = window.innerWidth - 80; // Leave 40px margin on each side
		const maxHeight = window.innerHeight - 80; // Leave 40px margin on top and bottom
		width = Math.min(width, maxWidth);
		height = Math.min(height, maxHeight);

		// Initialize position
		const defaultSnapPoint =
			snapPoints.find((p) => p.name === defaults.snapPosition) || snapPoints[0];
		if (defaultSnapPoint) {
			const { offsetX, offsetY } = snapManager.getAlignmentOffset(
				defaultSnapPoint.x,
				defaultSnapPoint.y,
				width,
				height
			);

			x = defaultSnapPoint.x + offsetX;
			y = defaultSnapPoint.y + offsetY;
			currentSnapPoint = defaultSnapPoint;
		}

		// Connect drag handle element if provided
		if (dragHandleElement) {
			dragHandleElement.addEventListener('pointerdown', handleHeaderPointerDown);
		}

		window.addEventListener('resize', handleWindowResize);
		return () => {
			window.removeEventListener('resize', handleWindowResize);
			if (dragHandleElement) {
				dragHandleElement.removeEventListener('pointerdown', handleHeaderPointerDown);
			}
		};
	});
</script>

<div
	bind:this={modalEl}
	use:portal={'body'}
	use:focusable={{
		trap: true,
		activate: true,
		focusable: true,
		dim: true,
		onEsc: () => {
			onCancel?.();
		}
	}}
	class="floating-modal"
	class:snapping
	class:resizing={dragResizeHandler.isResizing}
	style:left={pxToRem(x, zoom) + 'rem'}
	style:top={pxToRem(y, zoom) + 'rem'}
	style:width={pxToRem(width, zoom) + 'rem'}
	style:height={pxToRem(height, zoom) + 'rem'}
	style:max-width="calc(100vw - 5rem)"
	style:max-height="calc(100vh - 5rem)"
>
	<ResizeHandles onResizeStart={handleResizeStart} snapPosition={currentSnapPoint?.name || ''} />
	{@render children()}
</div>

<style>
	.floating-modal {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-l);
		animation: slide-in 0.2s ease-out forwards;
	}

	@keyframes slide-in {
		from {
			transform: translateY(30px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	.floating-modal.snapping {
		transition:
			left 0.3s cubic-bezier(0.4, 0, 0.2, 1),
			top 0.3s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.floating-modal.resizing {
		transition: none;
	}
</style>
