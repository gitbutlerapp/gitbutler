import { test, expect } from '@playwright/test';

test.describe('Install script endpoint', () => {
	test('serves install.sh script', async ({ request }) => {
		const response = await request.get('/install.sh');

		// Verify response status
		expect(response.status()).toBe(200);

		// Verify content type
		const contentType = response.headers()['content-type'];
		expect(contentType).toContain('text/plain');

		// Verify the script content
		const scriptContent = await response.text();

		// Check for bash shebang
		expect(scriptContent).toContain('#!/bin/bash');

		// Check for key script sections to ensure it's the right file
		expect(scriptContent).toContain('GitButler CLI installation');
		expect(scriptContent).toContain('Detected platform:');
		expect(scriptContent).toContain('$HOME/Applications/GitButler.app');
		expect(scriptContent).toContain('$HOME/.local/bin');

		// Verify script has proper error handling
		expect(scriptContent).toContain('set -euo pipefail');

		// Verify it's executable bash (no obvious syntax errors in critical sections)
		expect(scriptContent).toContain('error()');
		expect(scriptContent).toContain('success()');
		expect(scriptContent).toContain('info()');
	});

	test('script has no-cache headers', async ({ request }) => {
		const response = await request.get('/install.sh');

		const cacheControl = response.headers()['cache-control'];
		expect(cacheControl).toBeTruthy();
		expect(cacheControl).toContain('no-cache');
		expect(cacheControl).toContain('no-store');
		expect(cacheControl).toContain('must-revalidate');
	});

	test('script can be downloaded with curl-like headers', async ({ request }) => {
		// Simulate curl request headers
		const response = await request.get('/install.sh', {
			headers: {
				'User-Agent': 'curl/7.64.1',
				Accept: '*/*'
			}
		});

		expect(response.status()).toBe(200);
		const scriptContent = await response.text();
		expect(scriptContent).toContain('#!/bin/bash');
	});
});
