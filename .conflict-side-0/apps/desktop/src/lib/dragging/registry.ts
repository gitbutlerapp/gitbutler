import { InjectionToken } from '@gitbutler/shared/context';
import type { Dropzone } from '$lib/dragging/dropzone';

export const DROPZONE_REGISTRY = new InjectionToken<DropzoneRegistry>('DropzoneRegistry');

/**
 * This class was bascially only created in order to facilitate injection.
 */
export class DropzoneRegistry {
	private map = new Map<HTMLElement, Dropzone>();
	get(key: HTMLElement) {
		return this.map.get(key);
	}
	set(key: HTMLElement, value: Dropzone) {
		this.map.set(key, value);
	}
	delete(key: HTMLElement) {
		this.map.delete(key);
	}
	values() {
		return this.map.values();
	}
}
