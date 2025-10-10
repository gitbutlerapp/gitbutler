import type { Action } from 'svelte/action';

interface StickyOptions {
	enabled?: boolean;
	onStuck?: (isStuck: boolean) => void;
	scrollThreshold?: number;
	scrollContainer?: Element | Window;
}

export function sticky(
	element: HTMLElement,
	options: StickyOptions
): ReturnType<Action<HTMLElement, StickyOptions>> {
	let { enabled = false, onStuck, scrollThreshold = 4, scrollContainer } = options;

	let isStuck = false;

	function cleanup() {
		if (scrollContainer) {
			scrollContainer.removeEventListener('scroll', handleScroll);
		}
	}

	function handleScroll() {
		if (!onStuck || !scrollContainer) return;

		let newIsStuck = false;

		// Get scroll position - works for both window and elements
		const scrollTop =
			scrollContainer === window ? window.scrollY : (scrollContainer as Element).scrollTop;
		const hasScrolled = scrollTop > scrollThreshold;

		if (hasScrolled) {
			// Only do expensive rect calculations when needed
			const containerRect =
				scrollContainer === window
					? { top: 0 }
					: (scrollContainer as Element).getBoundingClientRect();
			const elementRect = element.getBoundingClientRect();
			newIsStuck = Math.abs(elementRect.top - containerRect.top) <= 2;
		}

		if (newIsStuck !== isStuck) {
			isStuck = newIsStuck;
			onStuck(newIsStuck);
		}
	}

	function setup() {
		if (!enabled) return;

		// Apply styles in one batch to avoid layout thrashing
		Object.assign(element.style, {
			position: 'sticky',
			top: '0px',
			zIndex: 'var(--z-lifted)'
		});

		// Only setup scroll listener when callback is provided and scroll container exists
		if (onStuck && scrollContainer) {
			scrollContainer.addEventListener('scroll', handleScroll, { passive: true });
			handleScroll(); // Check initial state
		}
	}

	setup();

	return {
		update(newOptions: StickyOptions) {
			const wasEnabled = enabled;
			const oldScrollContainer = scrollContainer;

			// Merge options efficiently
			Object.assign(options, newOptions);
			({ enabled = true, onStuck, scrollThreshold = 4, scrollContainer } = options);

			if (!enabled) {
				cleanup();
				// Clear styles in one batch
				Object.assign(element.style, {
					position: '',
					top: '',
					zIndex: ''
				});
				return;
			}

			if (!wasEnabled) {
				setup();
			} else if (scrollContainer !== oldScrollContainer) {
				// Scroll container changed - update listener
				if (oldScrollContainer) {
					oldScrollContainer.removeEventListener('scroll', handleScroll);
				}
				if (onStuck && scrollContainer) {
					scrollContainer.addEventListener('scroll', handleScroll, { passive: true });
					handleScroll();
				}
			}
		},
		destroy() {
			cleanup();
			// Clear styles in one batch
			Object.assign(element.style, {
				position: '',
				top: '',
				zIndex: ''
			});
		}
	};
}
