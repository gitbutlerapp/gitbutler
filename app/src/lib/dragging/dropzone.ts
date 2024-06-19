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
	private activated: boolean = false;
	private hovered: boolean = false;

	private registered: boolean = false;
	// In order to propperly deregister functions we need to use the same
	// reference so we must store the function after we bind the reference
	private registeredOnDrop?: (e: DragEvent) => any;
	private registeredOnDragEnter?: (e: DragEvent) => any;
	private registeredOnDragLeave?: (e: DragEvent) => any;

	private target!: HTMLElement;

	private data?: any;

	constructor(
		private configuration: DropzoneConfiguration,
		private rootNode: HTMLElement
	) {
		// Sets this.target
		this.setTarget();
	}

	async register(data: any) {
		this.data = data;

		if (!this.configuration.accepts(await this.data)) return;
		if (this.registered) {
			this.unregister();
		}
		this.registered = true;

		this.registerListeners();

		// Mark the dropzone as active
		setTimeout(() => {
			this.configuration.onActivationStart();
			this.activated = true;
		}, 10);
	}

	unregister() {
		// Mark as no longer active and ensure its not stuck in the hover state
		this.activated = false;
		this.configuration.onActivationEnd();
		this.configuration.onHoverEnd();

		this.unregisterListeners();

		this.registered = false;
	}

	// Designed to quickly swap out the configuration
	async reregister(configuration: DropzoneConfiguration) {
		this.unregisterListeners();

		// On the previous configuration, mark configuration as deactivated and unhovered
		this.configuration.onActivationEnd();
		this.configuration.onHoverEnd();

		this.configuration = configuration;
		this.setTarget();

		if (!this.configuration.accepts(await this.data)) return;

		if (this.hovered) {
			this.configuration.onHoverStart();
		} else {
			this.configuration.onHoverEnd();
		}

		if (this.activated) {
			this.configuration.onActivationStart();
		} else {
			this.configuration.onActivationEnd();
		}
		this.registerListeners();
	}

	private registerListeners() {
		this.registeredOnDrop = this.onDrop.bind(this);
		this.registeredOnDragEnter = this.onDragEnter.bind(this);
		this.registeredOnDragLeave = this.onDragLeave.bind(this);

		this.target.addEventListener('drop', this.registeredOnDrop);
		this.target.addEventListener('dragenter', this.registeredOnDragEnter);
		this.target.addEventListener('dragleave', this.registeredOnDragLeave);
	}

	private unregisterListeners() {
		if (this.registeredOnDrop) {
			this.target.removeEventListener('drop', this.registeredOnDrop);
		}
		if (this.registeredOnDragEnter) {
			this.target.removeEventListener('dragenter', this.registeredOnDragEnter);
		}
		if (this.registeredOnDragLeave) {
			this.target.removeEventListener('dragleave', this.registeredOnDragLeave);
		}
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
		if (!this.activated) return;
		this.configuration.onDrop(await this.data);
	}

	private onDragEnter(e: DragEvent) {
		e.preventDefault();
		if (!this.activated) return;

		this.hovered = true;
		this.configuration.onHoverStart();
	}

	private onDragLeave(e: DragEvent) {
		e.preventDefault();
		if (!this.activated) return;

		this.hovered = false;
		this.configuration.onHoverEnd();
	}
}

export const dropzoneRegistry = new Map<HTMLElement, Dropzone>();

export function dropzone(node: HTMLElement, configuration: DropzoneConfiguration) {
	function setup(configuration: DropzoneConfiguration) {
		if (configuration.disabled) return;

		if (dropzoneRegistry.has(node)) {
			clean();
		}

		dropzoneRegistry.set(node, new Dropzone(configuration, node));
	}

	function clean() {
		dropzoneRegistry.get(node)?.unregister();
		dropzoneRegistry.delete(node);
	}

	setup(configuration);

	function update(configuration: DropzoneConfiguration) {
		const dropzone = dropzoneRegistry.get(node);

		if (dropzone) {
			dropzone.reregister(configuration);
		} else {
			setup(configuration);
		}
	}

	return {
		update(configuration: DropzoneConfiguration) {
			update(configuration);
		},
		destroy() {
			clean();
		}
	};
}
