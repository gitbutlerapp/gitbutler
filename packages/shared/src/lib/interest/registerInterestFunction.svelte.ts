import type { Interest } from '$lib/interest/intrestStore';

export type InView = {
	element?: HTMLElement;
};

export function registerInterest(interest: Interest, inView?: InView) {
	if (inView) {
		registerInterestInView(interest, inView.element);
	} else {
		registerInterestAlways(interest);
	}
}

export function registerInterestAlways(interest: Interest) {
	$effect(() => {
		const unsubscribe = interest._subscribe();

		return unsubscribe;
	});
}

export function registerInterestInView(interest: Interest, element?: HTMLElement) {
	let inView = $state(false);

	$effect(() => {
		if (element) {
			inView = false;

			const observer = new IntersectionObserver(
				(entries) => {
					inView = entries[0]?.isIntersecting || false;
				},
				{
					root: null
				}
			);

			observer.observe(element);

			return () => {
				inView = false;
				observer.disconnect();
			};
		} else {
			inView = false;
		}
	});

	$effect(() => {
		if (inView) {
			const unsubscribe = interest._subscribe();

			// It is vitally important that we return the unsubscribe function
			return unsubscribe;
		}
	});
}
