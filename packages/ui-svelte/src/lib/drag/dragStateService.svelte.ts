import { InjectionToken } from "@gitbutler/core/context";
import { writable, type Readable } from "svelte/store";

export const DRAG_STATE_SERVICE: InjectionToken<DragStateService> = new InjectionToken(
	"DragStateService",
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
	private activeLabels = new Map<symbol, string | undefined>();

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
		const token = Symbol("dropLabel");
		this.activeLabels.set(token, label);
		this.updateDropLabel();
		return token;
	}

	clearDropLabel(token: symbol): void {
		// Remove the token from active labels
		if (this.activeLabels.delete(token)) {
			this.updateDropLabel();
		}
	}

	private updateDropLabel(): void {
		// Get the most recently added label (Map maintains insertion order)
		// Note: If the same token is updated, it won't change position in the Map
		const labels = Array.from(this.activeLabels.values());
		const currentLabel = labels.at(-1);
		this._dropLabel.set(currentLabel);
	}

	private endDragging(): void {
		this.dragCount = Math.max(0, this.dragCount - 1);

		if (this.dragCount === 0) {
			this._isDragging.set(false);
			this._dropLabel.set(undefined);
			this.activeLabels.clear();
		}
	}
}
