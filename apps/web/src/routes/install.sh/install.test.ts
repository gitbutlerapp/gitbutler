import installScript from '$scripts/install.sh?raw';
import { describe, it, expect } from 'vitest';

describe('Install script import', () => {
	it('successfully imports the install script', () => {
		expect(installScript).toBeDefined();
		expect(typeof installScript).toBe('string');
		expect(installScript.length).toBeGreaterThan(0);
	});

	it('contains bash shebang', () => {
		expect(installScript).toContain('#!/bin/bash');
	});

	it('contains critical installation steps', () => {
		// Verify key sections exist
		expect(installScript).toContain('Detected platform:');
		expect(installScript).toContain('$HOME/Applications/GitButler.app');
		expect(installScript).toContain('$HOME/.local/bin');
		expect(installScript).toContain('GitButler CLI installation completed');
	});

	it('has proper error handling', () => {
		expect(installScript).toContain('set -euo pipefail');
		expect(installScript).toContain('error()');
	});

	it('supports macOS platforms', () => {
		expect(installScript).toContain('darwin');
		expect(installScript).toContain('x86_64');
		expect(installScript).toContain('aarch64');
	});

	it('handles Fish shell', () => {
		expect(installScript).toContain('FISH_SHELL');
		expect(installScript).toContain('fish_add_path');
	});
});
