export type ClickOpts = { trigger?: HTMLElement; handler: () => void; enabled: boolean };

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
	return {
		destroy() {
			document.removeEventListener('click', onClick, true);
		},
		update(opts: ClickOpts) {
			document.removeEventListener('click', onClick, true);
			if (!opts.enabled) return;
			trigger = opts.trigger;
			document.addEventListener('click', onClick, true);
		}
	};
}
