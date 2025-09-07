<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import { FOCUS_MANAGER } from '@gitbutler/ui/focus/focusManager';

	const focusManager = inject(FOCUS_MANAGER);
	const { cursor: target, outline } = focusManager;
	const metadata = $derived(focusManager.getOptions($target));

	function findNearestRelativeDiv(element: HTMLElement): HTMLElement | undefined {
		let current = element.parentElement;

		while (current) {
			if (current === document.body) {
				return current;
			}
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
		const width = from.offsetWidth;
		const height = from.offsetHeight;

		to.style.left = left + 'px';
		to.style.top = top + 'px';

		to.style.width = width + 'px';
		to.style.height = height + 'px';
	}

	$effect(() => {
		if (!$target || !$outline || metadata?.dim) {
			return;
		}

		const element = createCursor($target);
		copyPosition($target, element);

		const observer = new ResizeObserver(() => {
			if ($target && element.isConnected) {
				copyPosition($target, element);
			}
		});

		observer.observe($target);

		return () => {
			element.remove();
			observer.disconnect();
		};
	});
</script>

<style lang="postcss">
	:global(.focus-cursor) {
		z-index: var(--z-blocker);
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
