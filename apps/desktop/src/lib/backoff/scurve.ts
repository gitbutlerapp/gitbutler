export function scurveBackoff(ageMs: number, min: number, max: number): number {
	// S-curve (sigmoid) with y-axis intercept is at ~10 seconds and max value at 10 minutes.
	const delaySeconds =
		(max - min) / 1000 / (1 + Math.exp(-(ageMs / 1000 - 2000) / 300)) + min / 1000;
	return delaySeconds * 1000;
}
