export interface ToolTipOptions {
	text: string;
	delay?: number;
}

const defaultOptions: Partial<ToolTipOptions> = {
	delay: 1200
};

export function tooltip(node: HTMLElement, optsOrString: ToolTipOptions | string | undefined) {
	// The tooltip element we are adding to the dom
	let tooltip: HTMLDivElement | undefined;

	// Note that we use this both for delaying show, as well as delaying hide
	let timeoutId: any;

	// Options
	let { text, delay } = defaultOptions;

	// Most use cases only involve passing a string, so we allow either opts of
	// simple text.
	function setOpts(opts: ToolTipOptions | string | undefined) {
		if (typeof opts == 'string') {
			text = opts;
		} else {
			({ text, delay } = opts || {});
		}
	}

	setOpts(optsOrString);

	function onMouseOver() {
		// If tooltip is displayed we clear hide timeout
		if (tooltip && timeoutId) clearTimeout(timeoutId);
		// If no tooltip and no timeout id we set a show timeout
		else if (!tooltip && !timeoutId) timeoutId = setTimeout(() => show(), delay);
	}

	function onMouseLeave() {
		// If tooltip shown when mouse out then we hide after delay
		if (tooltip) hideAfterDelay();
		// But if we mouse out before tooltip is shown, we cancel the show timer
		else if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = undefined;
		}
	}

	function show() {
		if (!text || !node.isConnected) return;
		tooltip = document.createElement('div') as HTMLDivElement;
		// TODO: Can we co-locate tooltip.js & tooltip.postcss?
		tooltip.classList.add('tooltip', 'text-base-11'); // see tooltip.postcss
		tooltip.innerText = text;
		document.body.appendChild(tooltip);
		adjustPosition();
	}

	function hide() {
		if (tooltip) tooltip.remove();
		tooltip = undefined;
		timeoutId = undefined;
	}

	function hideAfterDelay() {
		if (timeoutId) {
			clearTimeout(timeoutId);
		}
		timeoutId = setTimeout(() => hide(), 250);
	}

	function adjustPosition() {
		if (!tooltip) return;

		// Dimensions and position of target element
		const nodeRect = node.getBoundingClientRect();
		const nodeHeight = nodeRect.height;
		const nodeWidth = nodeRect.width;
		const nodeLeft = nodeRect.left;
		const nodeTop = nodeRect.top;

		// Padding
		const padding = 4;

		// Window dimensions
		const windowHeight = window.innerHeight;
		const windowWidth = window.innerWidth;

		const tooltipHeight = tooltip.offsetHeight;
		const tooltipWidth = tooltip.offsetWidth;

		const showBelow = windowHeight > nodeTop + nodeHeight + tooltipHeight + padding;

		// Note that we don't check if width of tooltip is wider than the window.

		if (showBelow) {
			tooltip.style.top = `${(nodeTop + nodeHeight + padding) / 16}rem`;
		} else {
			tooltip.style.top = `${(nodeTop - tooltipHeight - padding) / 16}rem`;
		}

		let leftPos = nodeLeft - (tooltipWidth - nodeWidth) / 2;
		if (leftPos < padding) leftPos = padding;
		if (leftPos + tooltipWidth > windowWidth) leftPos = windowWidth - tooltipWidth - padding;
		tooltip.style.left = `${leftPos / 16}rem`;
	}

	node.addEventListener('mouseover', onMouseOver);
	node.addEventListener('mouseleave', onMouseLeave);

	return {
		update(opts: ToolTipOptions | string | undefined) {
			setOpts(opts);
		},
		destroy() {
			tooltip?.remove();
			timeoutId && clearTimeout(timeoutId);
			node.removeEventListener('mouseover', onMouseOver);
			node.removeEventListener('mouseleave', onMouseLeave);
		}
	};
}
