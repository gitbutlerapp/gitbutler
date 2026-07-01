import type { ModalBounds } from "$lib/floating/types";

export class ResizeCalculator {
	private minWidth: number;
	private minHeight: number;

	constructor(minWidth = 520, minHeight = 330) {
		this.minWidth = minWidth;
		this.minHeight = minHeight;
	}

	calculateNewDimensions(
		dx: number,
		dy: number,
		direction: string,
		snapPosition: string,
		baseBounds: ModalBounds,
	): ModalBounds {
		const isCenterHorizontal = snapPosition.includes("center") || snapPosition === "center";
		const isCenterVertical = snapPosition.includes("center") || snapPosition === "center";

		let newX = baseBounds.x;
		let newY = baseBounds.y;
		let newWidth = baseBounds.width;
		let newHeight = baseBounds.height;

		// Handle horizontal resizing
		if (direction.includes("w") || direction === "w") {
			const result = this.calculateHorizontalResize(
				dx,
				"w",
				snapPosition,
				isCenterHorizontal,
				baseBounds,
			);
			newX = result.x;
			newWidth = result.width;
		}

		if (direction.includes("e") || direction === "e") {
			const result = this.calculateHorizontalResize(
				dx,
				"e",
				snapPosition,
				isCenterHorizontal,
				baseBounds,
			);
			newX = result.x;
			newWidth = result.width;
		}

		// Handle vertical resizing
		if (direction.includes("n") || direction === "n") {
			const result = this.calculateVerticalResize(
				dy,
				"n",
				snapPosition,
				isCenterVertical,
				baseBounds,
			);
			newY = result.y;
			newHeight = result.height;
		}

		if (direction.includes("s") || direction === "s") {
			const result = this.calculateVerticalResize(
				dy,
				"s",
				snapPosition,
				isCenterVertical,
				baseBounds,
			);
			newY = result.y;
			newHeight = result.height;
		}

		return { x: newX, y: newY, width: newWidth, height: newHeight };
	}

	private calculateHorizontalResize(
		dx: number,
		edge: "w" | "e",
		snapPosition: string,
		isCenterHorizontal: boolean,
		baseBounds: ModalBounds,
	) {
		let newX = baseBounds.x;
		let newWidth = baseBounds.width;

		if (edge === "w") {
			if (isCenterHorizontal && !snapPosition.includes("left") && !snapPosition.includes("right")) {
				// Center position - grow both directions
				const widthChange = -dx * 2;
				newWidth = Math.max(this.minWidth, baseBounds.width + widthChange);
				newX = baseBounds.x + (baseBounds.width - newWidth) / 2;
			} else if (snapPosition.includes("right")) {
				// Right-anchored
				newX = baseBounds.x + dx;
				newWidth = Math.max(this.minWidth, baseBounds.width - dx);
			} else {
				// Left-anchored or default
				newX = baseBounds.x + dx;
				newWidth = Math.max(this.minWidth, baseBounds.width - dx);
			}
		} else {
			// edge === 'e'
			if (isCenterHorizontal && !snapPosition.includes("left") && !snapPosition.includes("right")) {
				// Center position - grow both directions
				newWidth = Math.max(this.minWidth, baseBounds.width + dx * 2);
				newX = baseBounds.x - (newWidth - baseBounds.width) / 2;
			} else if (snapPosition.includes("left")) {
				// Left-anchored
				newWidth = Math.max(this.minWidth, baseBounds.width + dx);
			} else {
				// Right-anchored or default
				newWidth = Math.max(this.minWidth, baseBounds.width + dx);
			}
		}

		return { x: newX, width: newWidth };
	}

	private calculateVerticalResize(
		dy: number,
		edge: "n" | "s",
		snapPosition: string,
		isCenterVertical: boolean,
		baseBounds: ModalBounds,
	) {
		let newY = baseBounds.y;
		let newHeight = baseBounds.height;

		if (edge === "n") {
			if (isCenterVertical && !snapPosition.includes("top") && !snapPosition.includes("bottom")) {
				// Center position - grow both directions
				const heightChange = -dy * 2;
				newHeight = Math.max(this.minHeight, baseBounds.height + heightChange);
				newY = baseBounds.y + (baseBounds.height - newHeight) / 2;
			} else if (snapPosition.includes("bottom")) {
				// Bottom-anchored
				newY = baseBounds.y + dy;
				newHeight = Math.max(this.minHeight, baseBounds.height - dy);
			} else {
				// Top-anchored or default
				newY = baseBounds.y + dy;
				newHeight = Math.max(this.minHeight, baseBounds.height - dy);
			}
		} else {
			// edge === 's'
			if (isCenterVertical && !snapPosition.includes("top") && !snapPosition.includes("bottom")) {
				// Center position - grow both directions
				newHeight = Math.max(this.minHeight, baseBounds.height + dy * 2);
				newY = baseBounds.y - (newHeight - baseBounds.height) / 2;
			} else if (snapPosition.includes("top")) {
				// Top-anchored
				newHeight = Math.max(this.minHeight, baseBounds.height + dy);
			} else {
				// Bottom-anchored or default
				newHeight = Math.max(this.minHeight, baseBounds.height + dy);
			}
		}

		return { y: newY, height: newHeight };
	}
}
