import { InjectionToken } from "@gitbutler/core/context";
import type { Dropzone } from "$lib/dragging/dropzone";

export const DROPZONE_REGISTRY = new InjectionToken<DropzoneRegistry>("DropzoneRegistry");

/**
 * Registry for all active dropzones. Tracks active drag state so that
 * dropzones created mid-drag (e.g. due to data refresh) are automatically
 * activated, and so that all dropzones can be reliably deactivated when
 * a drag ends — even if the dragged element is destroyed before mouseup.
 */
export class DropzoneRegistry {
	private map = new Map<HTMLElement, Dropzone>();
	private activeDragData: unknown = undefined;
	private _isDragging = false;

	get(key: HTMLElement) {
		return this.map.get(key);
	}

	set(key: HTMLElement, value: Dropzone) {
		this.map.set(key, value);
		// Auto-activate dropzones registered during an active drag.
		if (this._isDragging) {
			value.activate(this.activeDragData);
		}
	}

	delete(key: HTMLElement) {
		this.map.delete(key);
	}

	has(key: HTMLElement) {
		return this.map.has(key);
	}

	values() {
		return this.map.values();
	}

	entries() {
		return this.map.entries();
	}

	/**
	 * Called when a drag operation starts. Activates all registered
	 * dropzones and stores the drag data so that late-registered
	 * dropzones can be activated automatically.
	 */
	startDrag(data: unknown) {
		this._isDragging = true;
		this.activeDragData = data;
		for (const dropzone of this.map.values()) {
			dropzone.activate(data);
		}
	}

	/**
	 * Called when a drag operation ends (mouseup or cleanup).
	 * Deactivates all dropzones and clears drag state.
	 */
	endDrag() {
		this._isDragging = false;
		this.activeDragData = undefined;
		for (const dropzone of this.map.values()) {
			dropzone.deactivate();
		}
	}

	get isDragging() {
		return this._isDragging;
	}
}
