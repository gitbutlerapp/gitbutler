import { InjectionToken } from '@gitbutler/core/context';
import { writable, type Readable } from 'svelte/store';

export const DRAG_STATE_SERVICE: InjectionToken<DragStateService> = new InjectionToken(
	'DragStateService'
);

/**
 * Centralized drag state service that tracks when any type of dragging is happening.
 * This service allows components to subscribe to drag state changes and enables
 * workspace-wide features like auto-panning when dragging commits, files, or hunks.
 */
export class DragStateService {
	private dragCount = 0;
	private readonly _isDragging = writable(false);
	private readonly _dropLabel = writable<string | undefined>(undefined);
	private currentDropLabel: string | undefined = undefined;
	private currentLabelToken: symbol | undefined = undefined;

	get isDragging(): Readable<boolean> {
		return this._isDragging;
	}

	get dropLabel(): Readable<string | undefined> {
		return this._dropLabel;
	}

	startDragging(): () => void {
		this.dragCount++;

		// If this is the first drag operation, update the state
		if (this.dragCount === 1) {
			this._isDragging.set(true);
		}

		// Return cleanup function
		return () => this.endDragging();
	}

	setDropLabel(label: string | undefined): symbol {
		const token = Symbol('dropLabel');
		this.currentLabelToken = token;
		this.currentDropLabel = label;
		this._dropLabel.set(label);
		return token;
	}

	clearDropLabel(token: symbol): void {
		// Only clear if this token is the current one
		if (this.currentLabelToken === token) {
			this.currentDropLabel = undefined;
			this.currentLabelToken = undefined;
			this._dropLabel.set(undefined);
		}
	}

	private endDragging(): void {
		this.dragCount = Math.max(0, this.dragCount - 1);

		if (this.dragCount === 0) {
			this._isDragging.set(false);
			this._dropLabel.set(undefined);
			this.currentDropLabel = undefined;
			this.currentLabelToken = undefined;
		}
	}
}
