import { InjectionToken } from '@gitbutler/shared/context';

export type ResizeCallback = (value: number) => void;

export const RESIZE_SYNC = new InjectionToken<ResizeSync>('ResizeSync');

/**
 * The `Resizer` component uses this class for shift + drag synchronizing
 * widths across multiple instances.
 */
export class ResizeSync {
	private map: Record<string, [symbol, ResizeCallback][]> = {};

	subscribe(args: { key: string; resizerId: symbol; callback: ResizeCallback }) {
		const { key, resizerId: id, callback } = args;
		let callbacks = this.map[key];
		if (callbacks === undefined) {
			callbacks = [];
			this.map[key] = callbacks;
		}
		const item = [id, callback] as [symbol, ResizeCallback];
		callbacks.push(item);
		return () => {
			callbacks.splice(callbacks.indexOf(item));
		};
	}

	emit(key: string, exclude: symbol, value: number) {
		const callbacks = this.map[key];
		if (!callbacks) return;
		for (const [id, callback] of callbacks) {
			if (id !== exclude) {
				callback(value);
			}
		}
	}
}
