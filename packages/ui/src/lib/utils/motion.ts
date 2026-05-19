const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";
const REDUCED_MOTION_DURATION_MS = 1;

export const motionDurations = {
	fast: 120,
	medium: 180,
	slow: 240,
	overlay: 220,
	overlayExit: 160,
	layout: 200,
	loading: 900,
} as const;

function canUseMatchMedia() {
	return typeof window !== "undefined" && typeof window.matchMedia === "function";
}

export function prefersReducedMotion() {
	return canUseMatchMedia() && window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

export function getMotionDuration(duration: number) {
	return prefersReducedMotion() ? REDUCED_MOTION_DURATION_MS : duration;
}

export function getMotionDelay(delay: number) {
	return prefersReducedMotion() ? 0 : delay;
}

export function getMotionDistance(distance: number) {
	return prefersReducedMotion() ? 0 : distance;
}

export const reducedMotionQuery = REDUCED_MOTION_QUERY;
