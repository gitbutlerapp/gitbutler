import { isContentEditable } from '$lib/focus/utils';
import type { NavigationAction } from '$lib/focus/focusTypes';

export function getNavigationAction(key: string): NavigationAction | null {
	const keyMap: Record<string, NavigationAction> = {
		Tab: 'tab',
		ArrowLeft: 'left',
		ArrowRight: 'right',
		ArrowUp: 'up',
		ArrowDown: 'down'
	};
	return keyMap[key] ?? null;
}

export function isInputElement(target: EventTarget | null): boolean {
	return (
		(target instanceof HTMLElement && isContentEditable(target)) ||
		target instanceof HTMLTextAreaElement ||
		target instanceof HTMLInputElement
	);
}

export function getElementDescription(element: HTMLElement | undefined): string {
	if (!element) return '(none)';

	const tag = element.tagName.toLowerCase();
	const classes = element.className
		? `.${element.className
				.split(' ')
				.filter((c) => c)
				.join('.')}`
		: '';
	const htmlId = element.id ? `#${element.id}` : '';

	const maxClassLength = 50;
	const displayClasses =
		classes.length > maxClassLength ? classes.substring(0, maxClassLength) + '...' : classes;

	return `${tag}${htmlId}${displayClasses}`.trim() || tag;
}
