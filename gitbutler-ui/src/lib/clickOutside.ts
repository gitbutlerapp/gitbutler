export type ClickOpts = { trigger?: HTMLElement; handler: () => void; enabled?: boolean };

export function clickOutside(
	node: HTMLElement,
	params: ClickOpts
): { destroy: () => void; update: (opts: ClickOpts) => void } {
	let trigger: HTMLElement | undefined;
	function onClick(event: MouseEvent) {
		if (
			node &&
			!node.contains(event.target as HTMLElement) &&
			!trigger?.contains(event.target as HTMLElement)
		) {
			params.handler();
		}
	}
	document.addEventListener('click', onClick, true);
	document.addEventListener('contextmenu', onClick, true);
	return {
		destroy() {
			document.removeEventListener('click', onClick, true);
			document.removeEventListener('contextmenu', onClick, true);
		},
		update(opts: ClickOpts) {
			document.removeEventListener('click', onClick, true);
			document.removeEventListener('contextmenu', onClick, true);
			if (opts.enabled !== undefined && !opts.enabled) return;
			trigger = opts.trigger;
			document.addEventListener('click', onClick, true);
			document.addEventListener('contextmenu', onClick, true);
		}
	};
}
