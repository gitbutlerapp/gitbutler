import { formatAsNonRelative } from './noRelativeImportPaths.js';
import { expect, test } from 'vitest';
import path from 'node:path';

test('formatAsNonRelative', () => {
	const testFixtureDirectory = path.join(process.cwd(), 'src/testFixture');

	// Path defined in Super-config
	expect(formatAsNonRelative(path.join(testFixtureDirectory, './foo'))).toBe('bam');

	// Overriden path
	// Base-config definition
	expect(formatAsNonRelative(path.join(testFixtureDirectory, './lib/foo'))).toBe(undefined);
	// Super-config definition
	expect(formatAsNonRelative(path.join(testFixtureDirectory, './lib/bar'))).toBe('overriden');

	// Sub-config
	expect(formatAsNonRelative(path.join(testFixtureDirectory, './src/bar'))).toBe('@/bar');
});
