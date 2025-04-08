/**
 * Setup the testing environment for component tests.
 */
export function setup() {
	const previousResizeObserver = global.ResizeObserver;
	global.ResizeObserver = class ResizeObserver {
		observe() {
			// do nothing
		}

		unobserve() {
			// do nothing
		}

		disconnect() {
			// do nothing
		}
	};

	const previousIntersectionObserver = global.IntersectionObserver;
	global.IntersectionObserver = class IntersectionObserver {
		observe() {
			// do nothing
		}

		unobserve() {
			// do nothing
		}

		disconnect() {
			// do nothing
		}

		takeRecords() {
			return [];
		}

		root = null;
		rootMargin = '';
		thresholds = [];
	};

	return () => {
		global.ResizeObserver = previousResizeObserver;
		global.IntersectionObserver = previousIntersectionObserver;
	};
}
