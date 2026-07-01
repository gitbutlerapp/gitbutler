export type SnapPositionName =
	| "center"
	| "top-left"
	| "top-right"
	| "bottom-left"
	| "bottom-right"
	| "top-center"
	| "bottom-center"
	| "left-center"
	| "right-center";

export interface SnapPoint {
	x: number;
	y: number;
	name: SnapPositionName;
}

export interface ModalBounds {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface DragState {
	isDragging: boolean;
	startX: number;
	startY: number;
	baseX: number;
	baseY: number;
}

export interface ResizeState {
	isResizing: boolean;
	direction: string;
	startX: number;
	startY: number;
	baseX: number;
	baseY: number;
	baseWidth: number;
	baseHeight: number;
}
