import type { DropResult } from "$lib/dragging/dropResult";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { DropzoneRegistry } from "$lib/dragging/registry";

export type HoverArgs = {
	handler?: DropzoneHandler;
};

export interface DropzoneConfiguration {
	disabled: boolean;
	handlers: DropzoneHandler[];
	onActivationStart: () => void;
	onActivationEnd: () => void;
	onHoverStart: (args: HoverArgs) => void;
	onHoverEnd: () => void;
	onDropResult: (result: DropResult) => void;
	target: string;
	registry: DropzoneRegistry;
}

export class Dropzone {
	private activated: boolean = false;
	private hovered: boolean = false;
	private registered: boolean = false;
	private target!: HTMLElement;
	private data?: unknown;

	private boundOnDrop: (e: DragEvent) => void;
	private boundOnDragEnter: (e: DragEvent) => void;
	private boundOnDragLeave: (e: DragEvent) => void;
	private boundOnMouseUp: (e: MouseEvent) => void;
	private boundOnDragOver: (e: DragEvent) => void;

	constructor(
		private configuration: DropzoneConfiguration,
		private rootNode: HTMLElement,
	) {
		this.boundOnDrop = (e) => void this.onDrop(e);
		this.boundOnDragEnter = (e) => this.onDragEnter(e);
		this.boundOnDragLeave = (e) => this.onDragLeave(e);
		this.boundOnMouseUp = (e) => void this.onMouseUp(e);
		this.boundOnDragOver = (e) => this.onDragOver(e);

		this.setTarget();
	}

	activate(dropData: unknown) {
		this.data = dropData;
		if (!this.acceptedHandler) return;
		if (this.registered) this.deactivate();

		this.registered = true;
		this.registerListeners();

		// Activate immediately for pointer-based drags (no need for setTimeout with new system)
		this.configuration.onActivationStart();
		this.activated = true;
	}

	reactivate(newConfig: DropzoneConfiguration) {
		if (this.registered) {
			this.unregisterListeners();
		}

		this.configuration = newConfig;
		this.setTarget();
		this.registerListeners();

		if (this.activated) {
			this.configuration.onActivationStart();
		}
		if (this.hovered) {
			this.configuration.onHoverStart({ handler: this.acceptedHandler });
		}
	}

	deactivate() {
		if (this.registered) this.unregisterListeners();

		if (this.activated) this.configuration.onActivationEnd();
		this.activated = false;

		if (this.hovered) this.configuration.onHoverEnd();
		this.hovered = false;
	}

	triggerEnter() {
		if (!this.activated) return;
		if (this.hovered) return; // Already hovering
		this.hovered = true;
		this.configuration.onHoverStart({ handler: this.acceptedHandler });
	}

	triggerLeave() {
		if (!this.activated) return;
		if (!this.hovered) return; // Not hovering
		this.hovered = false;
		this.configuration.onHoverEnd();
	}

	getTarget() {
		return this.target;
	}

	private registerListeners() {
		// Support both native drag events and pointer-based events
		this.target.addEventListener("drop", this.boundOnDrop);
		this.target.addEventListener("dragenter", this.boundOnDragEnter);
		this.target.addEventListener("dragleave", this.boundOnDragLeave);
		this.target.addEventListener("dragover", this.boundOnDragOver);
		this.target.addEventListener("mouseup", this.boundOnMouseUp);
		this.registered = true;
	}

	private unregisterListeners() {
		this.target.removeEventListener("drop", this.boundOnDrop);
		this.target.removeEventListener("dragenter", this.boundOnDragEnter);
		this.target.removeEventListener("dragleave", this.boundOnDragLeave);
		this.target.removeEventListener("dragover", this.boundOnDragOver);
		this.target.removeEventListener("mouseup", this.boundOnMouseUp);
		this.registered = false;
	}

	private setTarget() {
		const child = this.rootNode.querySelector<HTMLElement>(this.configuration.target);

		if (child) {
			this.target = child;
		} else {
			this.target = this.rootNode;
		}
	}

	private async onDrop(e: DragEvent) {
		e.preventDefault();
		e.stopPropagation();
		if (!this.activated) return;
		await this.invokeHandler();
	}

	private async onMouseUp(e: MouseEvent) {
		// Handle drop via pointer events (mouseup)
		if (!this.activated) return;

		e.preventDefault();
		// Don't call stopPropagation() here - the draggable needs the mouseup event
		// to reach the window listener so it can clean up the drag clone

		// Note: we intentionally do NOT gate on this.hovered here. The mouseup
		// listener is registered on this.target, so it only fires when the mouse
		// is released on this element or a descendant. The RAF-based position
		// observer can race with the mouseup event and clear hovered=false right
		// before this fires (the root cause of flaky drag-and-drop on CI).
		await this.invokeHandler();
	}

	private onDragOver(e: DragEvent) {
		// Required for native drag-and-drop to work
		if (!this.activated) return;
		e.preventDefault();
	}

	private onDragEnter(e: DragEvent) {
		e.preventDefault();
		if (!this.activated) return;

		// Prevent duplicate hover state
		if (this.hovered) return;

		this.hovered = true;
		this.configuration.onHoverStart({ handler: this.acceptedHandler });
	}

	private onDragLeave(e: DragEvent) {
		e.preventDefault();
		if (!this.activated) return;

		// For drag and mouse events, check if we're really leaving (not just entering a child)
		const relatedTarget = e.relatedTarget as Node | null;
		if (relatedTarget && this.target.contains(relatedTarget)) {
			return;
		}

		this.hovered = false;
		this.configuration.onHoverEnd();
	}

	private async invokeHandler(): Promise<void> {
		const handler = this.acceptedHandler;
		if (!handler) return;
		try {
			const result = await handler.ondrop(this.data);
			if (result) {
				this.configuration.onDropResult(result);
			}
		} catch (err) {
			this.configuration.onDropResult({
				type: "error",
				title: "Drop operation failed",
				error: err,
			});
		}
	}

	/** It is assumed at most one will accept the data. */
	private get acceptedHandler() {
		return this.configuration.handlers.find((h) => h.accepts(this.data));
	}
}

export function dropzone(node: HTMLElement, configuration: DropzoneConfiguration) {
	let instance: Dropzone | undefined;

	function setup(config: DropzoneConfiguration) {
		if (config.disabled) return;

		if (instance) {
			instance.deactivate();
		}

		instance = new Dropzone(config, node);
		configuration.registry.set(node, instance);
	}

	function cleanup() {
		if (instance) {
			instance.deactivate();
			instance = undefined;
		}
		configuration.registry.delete(node);
	}

	setup(configuration);

	return {
		update(newConfig: DropzoneConfiguration) {
			if (instance) {
				instance.reactivate(newConfig);
			} else {
				setup(newConfig);
			}
		},
		destroy() {
			cleanup();
		},
	};
}
