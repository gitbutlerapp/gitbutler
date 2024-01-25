export interface ToolTipOptions {
	text: string;
	// In the near future we'll need the ability to specify multiple
	// options for the tooltip, such as a hotkey.
	// hotkey?: string;
}

export function tooltip(node: HTMLElement, optsOrText: ToolTipOptions | string) {
	// The tooltip element we are adding to the dom
	let element: HTMLDivElement | undefined;

	// Text for the tooltip
	let text: string;

	// Note that we use this both for delaying show, as well as delaying hide
	let timeoutId: any;

	// Most use cases only involve passing a string, so we allow either opts of
	// simple text.
	if (typeof optsOrText == 'string') {
		text = optsOrText;
	} else {
		text = optsOrText.text;
	}

	if (!text) return;

	function onMouseOver() {
		// If tooltip is displayed we clear hide timeout
		if (element && timeoutId) clearTimeout(timeoutId);
		// If no tooltip and no timeout id we set a show timeout
		else if (!element && !timeoutId) timeoutId = setTimeout(() => show(), 1500);
	}

	function onMouseLeave() {
		// If tooltip shown when mouse out then we hide after delay
		if (element) hideAfterDelay();
		// But if we mouse out before tooltip is shown, we cancel the show timer
		else if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = undefined;
		}
	}

	function show() {
		element = document.createElement('div') as HTMLDivElement;
		// TODO: Can we co-locate tooltip.js & tooltip.postcss?
		element.classList.add('tooltip'); // see tooltip.postcss
		element.innerText = text;
		document.body.appendChild(element);
		adjustPosition();
	}

	function hide() {
		console.log('hide');
		if (element) element.remove();
		element = undefined;
		timeoutId = undefined;
	}

	function hideAfterDelay() {
		if (timeoutId) {
			clearTimeout(timeoutId);
		}
		timeoutId = setTimeout(() => hide(), 250);
	}

	function adjustPosition() {
		if (!element) return;

		// Dimensions and position of target element
		const nodeRect = node.getBoundingClientRect();

		// Padding
		const padding = 4;

		// Window dimensions
		const windowHeight = window.innerHeight;
		const windowWidth = window.innerWidth;

		const tipHeight = element.offsetHeight;
		const tipWidth = element.offsetWidth;

		const showBelow = windowHeight > nodeRect.top + nodeRect.height + tipHeight + padding;

		// Note that we don't check if width of tooltip is wider than the window.

		if (showBelow) {
			element.style.top = `${(nodeRect.top + nodeRect.height + padding) / 16}rem`;
		} else {
			element.style.top = `${(nodeRect.top - padding - tipHeight) / 16}rem`;
		}

		let leftPos = nodeRect.left - (tipWidth - nodeRect.width) / 2;
		if (leftPos < padding) leftPos = padding;
		if (leftPos + tipWidth > windowWidth) leftPos = windowWidth - tipWidth - padding;
		element.style.left = `${leftPos / 16}rem`;
	}

	console.log('listening');
	node.addEventListener('mouseover', onMouseOver);
	node.addEventListener('mouseleave', onMouseLeave);

	return {
		update(opts: ToolTipOptions) {
			({ text } = opts);
		},
		destroy() {
			element?.remove();
			timeoutId && clearTimeout(timeoutId);
			node.removeEventListener('mouseover', onMouseOver);
			node.removeEventListener('mouseleave', onMouseLeave);
		}
	};
}
