import { scurveBackoff } from './scurve';
import { expect, test } from 'vitest';

test('test s-curve backoff 1', () => {
	const min = 1000;
	const max = 20000;
	const initialDelay = scurveBackoff(0, min, max);
	expect(initialDelay).toBeGreaterThan(min);
	expect(initialDelay).toBeLessThan(1.1 * min);

	const finalDelay = scurveBackoff(Number.MAX_SAFE_INTEGER, min, max);
	expect(finalDelay).toEqual(max);
});

test('test s-curve backoff 2', () => {
	const min = 10000;
	const max = 600000;
	const initialDelay = scurveBackoff(0, min, max);
	expect(initialDelay).toBeGreaterThan(min);
	expect(initialDelay).toBeLessThan(1.1 * min);

	const finalDelay = scurveBackoff(Number.MAX_SAFE_INTEGER, min, max);
	expect(finalDelay).toEqual(max);
});
