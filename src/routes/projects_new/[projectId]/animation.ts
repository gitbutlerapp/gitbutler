import { spring } from 'svelte/motion';

const OPEN_SPRING = { stiffness: 0.1, damping: 0.4 };
const CLOSE_SPRING = { stiffness: 0.1, damping: 0.6 };

function getHeightSpring() {
	const heightSpring = spring(0, OPEN_SPRING);
	let firstTime = true;
	const sync = (height: number) => {
		heightSpring.set(height || 0, { hard: firstTime });
		firstTime = false;
	};
	return { sync, heightSpring };
}

export default function animateHeight(el: HTMLElement) {
	const { heightSpring, sync } = getHeightSpring();

	let currentHeight = 0;
	const ro = new ResizeObserver(() => {
		const newHeight = el.offsetHeight;
		Object.assign(heightSpring, newHeight > currentHeight ? OPEN_SPRING : CLOSE_SPRING);
		currentHeight = newHeight;
		sync(el.offsetHeight);
	});

	const unsubscriber = heightSpring.subscribe((height) => {
		// when dragging, something sets height to 0 for some reason
		if (height !== 0) {
			if (el.parentNode) {
				const parent = el.parentNode as HTMLElement;
				parent.style.height = `${height}px`;
			}
		}
	});

	ro.observe(el);

	return {
		update() {},
		destroy() {
			ro.disconnect();
			unsubscriber();
		}
	};
}
