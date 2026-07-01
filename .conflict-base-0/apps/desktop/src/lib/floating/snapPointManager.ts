import type { SnapPoint, ModalBounds } from "$lib/floating/types";

export class SnapPointManager {
	private margin: number;

	constructor(margin = 40) {
		this.margin = margin;
	}

	calcSnapPoints(): SnapPoint[] {
		const w = window.innerWidth;
		const h = window.innerHeight;
		const { margin: m } = this;

		return [
			// Corners
			{ x: m, y: m, name: "top-left" },
			{ x: w - m, y: m, name: "top-right" },
			{ x: m, y: h - m, name: "bottom-left" },
			{ x: w - m, y: h - m, name: "bottom-right" },
			// Edge centers
			{ x: w / 2, y: m, name: "top-center" },
			{ x: w / 2, y: h - m, name: "bottom-center" },
			{ x: m, y: h / 2, name: "left-center" },
			{ x: w - m, y: h / 2, name: "right-center" },
			// Screen center
			{ x: w / 2, y: h / 2, name: "center" },
		];
	}

	getAlignmentOffset(snapX: number, snapY: number, modalWidth: number, modalHeight: number) {
		const w = window.innerWidth;
		const h = window.innerHeight;
		const { margin: m } = this;

		let offsetX = 0;
		let offsetY = 0;

		// Horizontal alignment
		if (snapX <= m) {
			offsetX = 0; // Left edge
		} else if (snapX >= w - m) {
			offsetX = -modalWidth; // Right edge
		} else {
			offsetX = -modalWidth / 2; // Center
		}

		// Vertical alignment
		if (snapY <= m) {
			offsetY = 0; // Top edge
		} else if (snapY >= h - m) {
			offsetY = -modalHeight; // Bottom edge
		} else {
			offsetY = -modalHeight / 2; // Center
		}

		return { offsetX, offsetY };
	}

	findNearestSnapPoint(
		modalCenterX: number,
		modalCenterY: number,
		snapPoints: SnapPoint[],
	): SnapPoint {
		return snapPoints.reduce((closest, point) => {
			const dist = Math.hypot(modalCenterX - point.x, modalCenterY - point.y);
			const closestDist = Math.hypot(modalCenterX - closest.x, modalCenterY - closest.y);
			return dist < closestDist ? point : closest;
		});
	}

	constrainToViewport(bounds: ModalBounds): { x: number; y: number } {
		const viewportWidth = window.innerWidth;
		const viewportHeight = window.innerHeight;
		const { margin: m } = this;

		const maxX = viewportWidth - bounds.width - m;
		const maxY = viewportHeight - bounds.height - m;

		let newX = Math.max(m, Math.min(bounds.x, maxX));
		let newY = Math.max(m, Math.min(bounds.y, maxY));

		// Allow extension to edges if modal is too large
		if (bounds.width > viewportWidth - m * 2) {
			newX = m;
		}
		if (bounds.height > viewportHeight - m * 2) {
			newY = m;
		}

		return { x: newX, y: newY };
	}
}
