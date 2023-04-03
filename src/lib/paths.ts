type Params = { separator: string; value: string };

export const collapsable = (e: HTMLElement, params: Params) => {
	if (e.textContent === null) return;
	e.dataset['value'] = e.textContent;

	const collapse = (e: HTMLElement, { separator, value }: Params) => {
		e.textContent = value;

		while (e.offsetWidth < e.scrollWidth) {
			const parts: string[] = e.textContent.split(separator);
			const firstLongPartIndex = parts.findIndex((p) => p.length > 1);
			if (firstLongPartIndex === -1) return;
			e.textContent = [
				...parts.slice(0, firstLongPartIndex),
				parts[firstLongPartIndex][0],
				...parts.slice(firstLongPartIndex + 1)
			].join(separator);
		}
	};

	collapse(e, params);

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};
	const onResize = () => collapse(e, params);
	window.addEventListener('resize', debounce(onResize, 300));
	return {
		update: (params: Params) => collapse(e, params),
		destroy: () => window.removeEventListener('resize', onResize)
	};
};
