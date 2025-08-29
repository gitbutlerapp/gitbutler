<script lang="ts">
	import { FOCUS_MANAGER } from '$lib/focus/focusManager';
	import { inject } from '@gitbutler/shared/context';

	const { cursor: target, outline } = inject(FOCUS_MANAGER);

	function findNearestRelativeDiv(element: HTMLElement): HTMLElement | undefined {
		let current = element.parentElement;

		while (current && current !== document.body) {
			if (isRelativePos(current)) {
				return current;
			}
			current = current.parentElement;
		}
	}

	function isRelativePos(element: HTMLElement) {
		const style = element.computedStyleMap();
		const position = style.get('position')?.toString();
		return position === 'relative';
	}

	function createCursor(target: HTMLElement) {
		const insertionPoint = findNearestRelativeDiv(target);
		const element = document.createElement('div');
		element.classList.add('focus-cursor');
		insertionPoint?.appendChild(element);
		return element;
	}

	function copyPosition(from: HTMLElement, to: HTMLElement) {
		const left = from.offsetLeft;
		const top = from.offsetTop;
		const width = from.clientWidth;
		const height = from.clientHeight;

		to.style.left = left ? left + 'px' : '0';
		to.style.top = top ? top + 'px' : '0';

		to.style.width = width + 'px';
		to.style.height = height + 'px';
	}

	$effect(() => {
		if ($target && $outline) {
			let element = createCursor($target);
			copyPosition($target, element);
			const observer = new ResizeObserver(() => {
				copyPosition($target, element);
			});
			observer.observe($target);
			return () => {
				element.remove();
				observer.disconnect();
			};
		}
	});
</script>

<style lang="postcss">
	:global(.focus-cursor) {
		z-index: var(--z-lifted);
		position: absolute;

		/* Focus outline frame */
		border: 2px solid var(--clr-btn-pop-outline);
		border-radius: var(--radius-ml);

		/* Transparent background - only outline frame */
		background: transparent;

		/* Initial state - hidden */
		pointer-events: none;

		/* Smooth transitions for position and size changes */
		transition: opacity 0.1s ease-in-out;
	}
</style>
