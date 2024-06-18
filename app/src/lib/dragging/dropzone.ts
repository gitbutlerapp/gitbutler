export interface DropzoneConfiguration {
	disabled: boolean;
	accepts: (data: any) => boolean;
	onDrop: (data: any) => Promise<void> | void;
	onActivationStart: () => void;
	onActivationEnd: () => void;
	onHoverStart: () => void;
	onHoverEnd: () => void;
	target: string;
}
export class Dropzone {
	private active: boolean = false;

	private registered: boolean = false;
	// In order to propperly deregister functions we need to use the same
	// reference so we must store the function after we bind the reference
	private registeredOnDrop?: (e: DragEvent) => any;
	private registeredOnDragEnter?: (e: DragEvent) => any;
	private registeredOnDragLeave?: (e: DragEvent) => any;

	private target: HTMLElement;

	constructor(
		private configuration: DropzoneConfiguration,
		node: HTMLElement
	) {
		const child = node.querySelector<HTMLElement>(configuration.target);

		if (child) {
			this.target = child;
		} else {
			this.target = node;
		}
	}

	async register(data: any) {
		if (!this.configuration.accepts(await data)) return;
		if (this.registered) {
			this.unregister();
		}

		// Register listeners and keep references to the functions
		this.registered = true;

		this.registeredOnDrop = async (e: DragEvent) => await this.onDrop(e, data);
		this.registeredOnDragEnter = this.onDragEnter.bind(this);
		this.registeredOnDragLeave = this.onDragLeave.bind(this);

		this.target.addEventListener('drop', this.registeredOnDrop);
		this.target.addEventListener('dragenter', this.registeredOnDragEnter);
		this.target.addEventListener('dragleave', this.registeredOnDragLeave);

		// Mark the dropzone as active
		setTimeout(() => {
			this.configuration.onActivationStart();
			this.active = true;
		}, 10);
	}

	unregister() {
		// Mark as no longer active and ensure its not stuck in the hover state
		this.active = false;
		this.configuration.onActivationEnd();
		this.configuration.onHoverEnd();

		// Unregister listeners
		if (this.registeredOnDrop) {
			this.target.removeEventListener('drop', this.registeredOnDrop);
		}
		if (this.registeredOnDragEnter) {
			this.target.removeEventListener('dragenter', this.registeredOnDragEnter);
		}
		if (this.registeredOnDragLeave) {
			this.target.removeEventListener('dragleave', this.registeredOnDragLeave);
		}

		this.registered = false;
	}

	private async onDrop(e: DragEvent, data: any) {
		e.preventDefault();
		if (!this.active) return;
		this.configuration.onDrop(await data);
	}

	private onDragEnter(e: DragEvent) {
		e.preventDefault();
		if (!this.active) return;
		this.configuration.onHoverStart();
	}

	private onDragLeave(e: DragEvent) {
		e.preventDefault();
		if (!this.active) return;
		this.configuration.onHoverEnd();
	}
}

export const dropzoneRegistry = new Map<HTMLElement, Dropzone>();

export function dropzone(node: HTMLElement, opts: DropzoneConfiguration) {
	function setup(opts: DropzoneConfiguration) {
		if (opts.disabled) return;

		if (dropzoneRegistry.has(node)) {
			clean();
		}

		dropzoneRegistry.set(node, new Dropzone(opts, node));
	}

	function clean() {
		dropzoneRegistry.get(node)?.unregister();
		dropzoneRegistry.delete(node);
	}

	setup(opts);

	return {
		update(opts: DropzoneConfiguration) {
			clean();
			setup(opts);
		},
		destroy() {
			clean();
		}
	};
}
