import { InjectionToken } from '@gitbutler/core/context';
import type { Dropzone } from '$lib/dragging/dropzone';

export const DROPZONE_REGISTRY = new InjectionToken<DropzoneRegistry>('DropzoneRegistry');

/**
 * Registry for dropzones that also tracks active drag state.
 * When a drag is active, newly registered dropzones are automatically
 * activated to ensure dropzones created during re-renders still work.
 */
export class DropzoneRegistry {
	private map = new Map<HTMLElement, Dropzone>();
	private currentDragData: unknown = undefined;
	private isDragActive = false;

	get(key: HTMLElement) {
		return this.map.get(key);
	}
	set(key: HTMLElement, value: Dropzone) {
		this.map.set(key, value);
		// If a drag is currently active, activate the newly added dropzone
		if (this.isDragActive) {
			value.activate(this.currentDragData);
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
	 * Called when a drag operation starts. Stores the drag data so that
	 * any dropzones added during the drag can be activated.
	 */
	startDrag(data: unknown) {
		this.isDragActive = true;
		this.currentDragData = data;
	}

	/**
	 * Called when a drag operation ends. Clears the drag state.
	 */
	endDrag() {
		this.isDragActive = false;
		this.currentDragData = undefined;
	}
}
