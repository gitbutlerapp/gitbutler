import { camelCaseToTitleCase } from './string';
import { describe, expect, test } from 'vitest';

describe.concurrent('Commit types handled correctly', () => {
	test('localAndRemote', () => {
		expect(camelCaseToTitleCase('localAndRemote')).toEqual('Local And Remote');
	});

	test('local', () => {
		expect(camelCaseToTitleCase('local')).toEqual('Local');
	});

	test('remote', () => {
		expect(camelCaseToTitleCase('remote')).toEqual('Remote');
	});

	test('localAndShadow', () => {
		expect(camelCaseToTitleCase('localAndShadow')).toEqual('Local And Shadow');
	});

	test('LocalAndShadow', () => {
		expect(camelCaseToTitleCase('LocalAndShadow')).toEqual('Local And Shadow');
	});
});
