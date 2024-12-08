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
	private target!: HTMLElement;
	private data?: any;

	private boundOnDrop: (e: DragEvent) => any;
	private boundOnDragEnter: (e: DragEvent) => any;
	private boundOnDragLeave: (e: DragEvent) => any;

	constructor(
		private configuration: DropzoneConfiguration,
		private rootNode: HTMLElement
	) {
		this.boundOnDrop = this.onDrop.bind(this);
		this.boundOnDragEnter = this.onDragEnter.bind(this);
		this.boundOnDragLeave = this.onDragLeave.bind(this);

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
		this.activated = true;
		setTimeout(() => {
			this.configuration.onActivationStart();
		}, 10);
	}
	async reregister(newConfig: DropzoneConfiguration) {
		if (this.registered) {
			this.unregisterListeners();
		}

		this.configuration = newConfig;
		this.setTarget();

		if (!this.configuration.accepts(await this.data)) {
			this.registerListeners();

			if (this.activated) {
				this.configuration.onActivationStart();
			}

			if (this.hovered) {
				this.configuration.onHoverStart();
			}
		}
	}

	unregister() {
		if (this.registered) {
			this.unregisterListeners();
		}
		this.activated = false;
		this.hovered = false;
		this.configuration.onActivationEnd();
		this.configuration.onHoverEnd();
	}

	private registerListeners() {
		this.target.addEventListener('drop', this.boundOnDrop);
		this.target.addEventListener('dragenter', this.boundOnDragEnter);
		this.target.addEventListener('dragleave', this.boundOnDragLeave);
		this.registered = true;
	}

	private unregisterListeners() {
		this.target.removeEventListener('drop', this.boundOnDrop);
		this.target.removeEventListener('dragenter', this.boundOnDragEnter);
		this.target.removeEventListener('dragleave', this.boundOnDragLeave);
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
	let instance: Dropzone | undefined;

	function setup(config: DropzoneConfiguration) {
		if (config.disabled) return;

		if (instance) {
			instance.unregister();
		}

		instance = new Dropzone(config, node);
		dropzoneRegistry.set(node, instance);
	}

	function cleanup() {
		if (instance) {
			instance.unregister();
			instance = undefined;
		}
		dropzoneRegistry.delete(node);
	}

	setup(configuration);

	return {
		update(newConfig: DropzoneConfiguration) {
			if (newConfig.disabled) {
				cleanup();
				return;
			}

			if (instance) {
				instance.reregister(newConfig);
			} else {
				setup(newConfig);
			}
		},
		destroy() {
			cleanup();
		}
	};
}
