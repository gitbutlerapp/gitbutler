import * as Filters from './filters';
import { expect, test, describe } from 'vitest';

describe.concurrent('unique', () => {
	test('When provided an array with duplicate values, it returns an array with unique values', () => {
		const array = [1, 2, 3, 1, 2, 3, 4, 5];
		const expected = [1, 2, 3, 4, 5];

		expect(array.filter(Filters.unique)).toEqual(expected);
	});

	test('When provided an array with no duplicate values, it returns the original array', () => {
		const array = [1, 2, 3, 4, 5];
		const expected = [1, 2, 3, 4, 5];

		expect(array.filter(Filters.unique)).toEqual(expected);
	});
});

describe.concurrent('uniqeByPropValues', () => {
	class Test {
		constructor(
			public id: number,
			public name: string
		) {}
		doSomething() {
			return this.id;
		}
	}

	test('When provided an array of objects with duplicate values, it returns an array with unique values', () => {
		const array = [
			{ id: 1, name: 'foo' },
			{ id: 2, name: 'bar' },
			{ id: 1, name: 'foo' },
			{ id: 2, name: 'bar' },
			{ id: 3, name: 'baz' }
		];
		const expected = [
			{ id: 1, name: 'foo' },
			{ id: 2, name: 'bar' },
			{ id: 3, name: 'baz' }
		];

		expect(array.filter(Filters.uniqeByPropValues)).toEqual(expected);
	});

	test('When provided an array of objects with duplicate values, it returns an array with unique values - classes', () => {
		const array = [
			new Test(1, 'foo'),
			new Test(2, 'bar'),
			new Test(1, 'foo'),
			new Test(2, 'bar'),
			new Test(3, 'baz')
		];
		const expected = [new Test(1, 'foo'), new Test(2, 'bar'), new Test(3, 'baz')];

		expect(array.filter(Filters.uniqeByPropValues)).toEqual(expected);
	});

	test('When provided an array of objects with no duplicate values, it returns the original array', () => {
		const array = [
			{ id: 1, name: 'foo' },
			{ id: 2, name: 'bar' },
			{ id: 3, name: 'baz' }
		];
		const expected = [
			{ id: 1, name: 'foo' },
			{ id: 2, name: 'bar' },
			{ id: 3, name: 'baz' }
		];

		expect(array.filter(Filters.uniqeByPropValues)).toEqual(expected);
	});

	test('When provided an array of objects with no duplicate values, it returns the original array - classes', () => {
		const array = [new Test(1, 'foo'), new Test(2, 'bar'), new Test(3, 'baz')];
		const expected = [new Test(1, 'foo'), new Test(2, 'bar'), new Test(3, 'baz')];

		expect(array.filter(Filters.uniqeByPropValues)).toEqual(expected);
	});
});
