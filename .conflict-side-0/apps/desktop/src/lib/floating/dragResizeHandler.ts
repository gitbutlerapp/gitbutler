import type { ResizeCalculator } from "$lib/floating/resizeCalculator";
import type { SnapPointManager } from "$lib/floating/snapPointManager";
import type { ModalBounds, DragState, ResizeState } from "$lib/floating/types";

export class DragResizeHandler {
	private snapManager: SnapPointManager;
	private resizeCalculator: ResizeCalculator;
	private dragState: DragState = {
		isDragging: false,
		startX: 0,
		startY: 0,
		baseX: 0,
		baseY: 0,
	};
	private resizeState: ResizeState = {
		isResizing: false,
		direction: "",
		startX: 0,
		startY: 0,
		baseX: 0,
		baseY: 0,
		baseWidth: 0,
		baseHeight: 0,
	};

	constructor(snapManager: SnapPointManager, resizeCalculator: ResizeCalculator) {
		this.snapManager = snapManager;
		this.resizeCalculator = resizeCalculator;
	}

	startDrag(event: PointerEvent, currentBounds: ModalBounds) {
		this.dragState = {
			isDragging: true,
			startX: event.clientX,
			startY: event.clientY,
			baseX: currentBounds.x,
			baseY: currentBounds.y,
		};

		window.addEventListener("pointermove", this.handlePointerMove);
		window.addEventListener("pointerup", this.handlePointerUp, { once: true });
	}

	startResize(event: PointerEvent, direction: string, currentBounds: ModalBounds) {
		this.resizeState = {
			isResizing: true,
			direction,
			startX: event.clientX,
			startY: event.clientY,
			baseX: currentBounds.x,
			baseY: currentBounds.y,
			baseWidth: currentBounds.width,
			baseHeight: currentBounds.height,
		};

		window.addEventListener("pointermove", this.handlePointerMove);
		window.addEventListener("pointerup", this.handlePointerUp, { once: true });
	}

	private handlePointerMove = (e: PointerEvent) => {
		e.preventDefault();
		e.stopPropagation();

		if (this.dragState.isDragging) {
			const dx = e.clientX - this.dragState.startX;
			const dy = e.clientY - this.dragState.startY;

			this.onDrag?.({
				x: this.dragState.baseX + dx,
				y: this.dragState.baseY + dy,
				width: 0, // Will be filled by component
				height: 0,
			});
		} else if (this.resizeState.isResizing) {
			const dx = e.clientX - this.resizeState.startX;
			const dy = e.clientY - this.resizeState.startY;

			const baseBounds = {
				x: this.resizeState.baseX,
				y: this.resizeState.baseY,
				width: this.resizeState.baseWidth,
				height: this.resizeState.baseHeight,
			};

			const newBounds = this.resizeCalculator.calculateNewDimensions(
				dx,
				dy,
				this.resizeState.direction,
				this.currentSnapPosition || "",
				baseBounds,
			);

			this.onResize?.(newBounds);
		}
	};

	private handlePointerUp = () => {
		window.removeEventListener("pointermove", this.handlePointerMove);

		if (this.dragState.isDragging) {
			this.dragState.isDragging = false;
			this.onDragEnd?.();
		} else if (this.resizeState.isResizing) {
			this.resizeState.isResizing = false;
			this.resizeState.direction = "";
			this.onResizeEnd?.();
		}
	};

	get isDragging() {
		return this.dragState.isDragging;
	}
	get isResizing() {
		return this.resizeState.isResizing;
	}

	// Callbacks to be set by the component
	onDrag?: (bounds: ModalBounds) => void;
	onResize?: (bounds: ModalBounds) => void;
	onDragEnd?: () => void;
	onResizeEnd?: () => void;
	currentSnapPosition?: string;
}
