import { memoize } from "$lib/memoization";
import { describe, it, expect, vi } from "vitest";

describe("memoize", () => {
	it("should return the same result for the same arguments", () => {
		const fn = vi.fn((a: number, b: number) => a + b);
		const memoized = memoize(fn);

		expect(memoized(1, 2)).toBe(3);
		expect(memoized(1, 2)).toBe(3);
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it("should call the original function for different arguments", () => {
		const fn = vi.fn((a: number, b: number) => a * b);
		const memoized = memoize(fn);

		expect(memoized(2, 3)).toBe(6);
		expect(memoized(3, 2)).toBe(6);
		expect(fn).toHaveBeenCalledTimes(2);
	});

	it("should work with functions returning objects", () => {
		const fn = vi.fn((x: number) => ({ value: x }));
		const memoized = memoize(fn);

		const result1 = memoized(5);
		const result2 = memoized(5);
		expect(result1).toBe(result2);
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it("should handle no arguments", () => {
		const fn = vi.fn(() => 42);
		const memoized = memoize(fn);

		expect(memoized()).toBe(42);
		expect(memoized()).toBe(42);
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it("should cache based on argument values, not references", () => {
		const fn = vi.fn((obj: { a: number }) => obj.a);
		const memoized = memoize(fn);

		expect(memoized({ a: 1 })).toBe(1);
		expect(memoized({ a: 1 })).toBe(1);
		expect(fn).toHaveBeenCalledTimes(1); // Because JSON.stringify({a:1}) === JSON.stringify({a:1})
	});

	it("should cache correctly for primitive and array arguments", () => {
		const fn = vi.fn((arr: number[]) => arr.reduce((a, b) => a + b, 0));
		const memoized = memoize(fn);

		const arr = [1, 2, 3];
		expect(memoized(arr)).toBe(6);
		expect(memoized(arr)).toBe(6);
		expect(fn).toHaveBeenCalledTimes(1);
		expect(memoized([1, 2, 3])).toBe(6);
		expect(fn).toHaveBeenCalledTimes(1);
		expect(memoized([1, 2, 3, 4])).toBe(10);
		expect(fn).toHaveBeenCalledTimes(2);
	});
});
