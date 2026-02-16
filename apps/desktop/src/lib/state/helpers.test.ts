import { combineResults } from '$lib/state/helpers';
import { QueryStatus } from '@reduxjs/toolkit/query';
import { describe, expect, test } from 'vitest';
import type { CustomResult } from '$lib/state/butlerModule';

describe.concurrent('combineResults', () => {
	test('when passed one result, keep the same status', () => {
		const fulfilled = {
			data: 'foo',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const pending = {
			data: undefined,
			error: undefined,
			status: QueryStatus.pending
		} as CustomResult<any>;
		const rejected = {
			data: undefined,
			error: 'failure!',
			status: QueryStatus.rejected
		} as CustomResult<any>;
		const uninitialized = {
			data: undefined,
			error: undefined,
			status: QueryStatus.uninitialized
		} as CustomResult<any>;
		expect(combineResults(fulfilled)).toEqual({ ...fulfilled, data: ['foo'] });
		expect(combineResults(pending)).toEqual(pending);
		expect(combineResults(rejected)).toEqual(rejected);
		expect(combineResults(uninitialized)).toEqual(uninitialized);
	});

	test('rejected takes precedence over all', () => {
		const fulfilled = {
			data: 'foo',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const pending = {
			data: undefined,
			error: undefined,
			status: QueryStatus.pending
		} as CustomResult<any>;
		const rejected = {
			data: undefined,
			error: 'failure!',
			status: QueryStatus.rejected
		} as CustomResult<any>;
		const uninitialized = {
			data: undefined,
			error: undefined,
			status: QueryStatus.uninitialized
		} as CustomResult<any>;
		expect(combineResults(fulfilled, pending, rejected, uninitialized)).toEqual(rejected);
	});

	test('uninitialized takes precedence over fulfilled and pending', () => {
		const fulfilled = {
			data: 'foo',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const pending = {
			data: undefined,
			error: undefined,
			status: QueryStatus.pending
		} as CustomResult<any>;
		const uninitialized = {
			data: undefined,
			error: undefined,
			status: QueryStatus.uninitialized
		} as CustomResult<any>;
		expect(combineResults(fulfilled, pending, uninitialized)).toEqual(uninitialized);
	});

	test('pending takes precedence over fulfilled', () => {
		const fulfilled = {
			data: 'foo',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const pending = {
			data: undefined,
			error: undefined,
			status: QueryStatus.pending
		} as CustomResult<any>;
		expect(combineResults(fulfilled, pending)).toEqual(pending);
	});

	test('multiple fulfilled combines results in order', () => {
		const a = {
			data: 'a',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const b = {
			data: 'b',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;
		const c = {
			data: 'c',
			error: undefined,
			status: QueryStatus.fulfilled
		} as CustomResult<any>;

		expect(combineResults(a, b, c)).toEqual({
			data: ['a', 'b', 'c'],
			error: undefined,
			status: QueryStatus.fulfilled
		});
	});

	test('combining zero results returns undefined', () => {
		expect(combineResults()).toEqual(undefined);
	});
});
