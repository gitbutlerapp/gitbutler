import '@testing-library/jest-dom/vitest';

// https://github.com/testing-library/svelte-testing-library/issues/284#issuecomment-2082726160
Element.prototype.animate = () => ({
	// @ts-expect-error `Animation` execpted
	finished: Promise.resolve(),
	cancel: () => {},
	finish: () => {}
});
