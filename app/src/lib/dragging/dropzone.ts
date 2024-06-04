export interface Dropzone {
	disabled: boolean;
	active: string;
	hover: string;
	accepts: (data: any) => boolean;
	onDrop: (data: any) => Promise<void> | void;
}

const defaultDropzoneOptions: Dropzone = {
	disabled: false,
	active: 'dropzone-active',
	hover: 'dropzone-hover',
	accepts: (data) => data === 'default',
	onDrop: () => {}
};

export function dropzone(node: HTMLElement, opts: Partial<Dropzone> | undefined) {
	let currentOptions = { ...defaultDropzoneOptions, ...opts };

	function setup(opts: Partial<Dropzone> | undefined) {
		currentOptions = { ...defaultDropzoneOptions, ...opts };
		if (currentOptions.disabled) return;

		register(node, currentOptions);
	}

	function clean() {
		unregister(currentOptions);
	}

	setup(opts);

	return {
		update(opts: Partial<Dropzone> | undefined) {
			clean();
			setup(opts);
		},
		destroy() {
			clean();
		}
	};
}

export const dzRegistry: [HTMLElement, Dropzone][] = [];

function register(node: HTMLElement, dropzone: Dropzone) {
	dzRegistry.push([node, dropzone]);
}

function unregister(dropzone: Dropzone) {
	const index = dzRegistry.findIndex(([, dz]) => dz === dropzone);
	if (index >= 0) dzRegistry.splice(index, 1);
}
