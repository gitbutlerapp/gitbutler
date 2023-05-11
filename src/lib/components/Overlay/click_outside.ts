import type { ActionReturn } from 'svelte/action';

interface Attributes {
	'on:outclick': (e: CustomEvent<void>) => void;
}

export default (node: HTMLElement): ActionReturn<any, Attributes> => {
	const handleClick = (event: Event) => {
		if (event.target && !node.contains(event.target as Node | null)) {
			node.dispatchEvent(new CustomEvent('outclick'));
		}
	};

	document.addEventListener('click', handleClick, true);

	return {
		destroy() {
			document.removeEventListener('click', handleClick, true);
		}
	};
};
