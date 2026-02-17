import { test, expect } from "@playwright/test";

test.describe("Install script endpoint", () => {
	test("serves install.sh script", async ({ request }) => {
		const response = await request.get("/install.sh");

		// Verify response status
		expect(response.status()).toBe(200);

		// Verify content type
		const contentType = response.headers()["content-type"];
		expect(contentType).toContain("text/plain");

		// Verify the script content
		const scriptContent = await response.text();

		// Check for shell shebang
		expect(scriptContent).toContain("#!/bin/sh");

		// Check that it's the bootstrap script
		expect(scriptContent).toContain("GitButler installer bootstrap script");
		expect(scriptContent).toContain("https://app.gitbutler.com/installers/info");
		expect(scriptContent).toContain("https://releases.gitbutler.com");

		// Verify script has proper error handling
		expect(scriptContent).toContain("set -e");

		// Verify platform detection
		expect(scriptContent).toContain("uname");
		expect(scriptContent).toContain("darwin");
	});

	test("script has no-cache headers", async ({ request }) => {
		const response = await request.get("/install.sh");

		const cacheControl = response.headers()["cache-control"];
		expect(cacheControl).toBeTruthy();
		expect(cacheControl).toContain("no-cache");
		expect(cacheControl).toContain("no-store");
		expect(cacheControl).toContain("must-revalidate");
	});

	test("script can be downloaded with curl-like headers", async ({ request }) => {
		// Simulate curl request headers
		const response = await request.get("/install.sh", {
			headers: {
				"User-Agent": "curl/7.64.1",
				Accept: "*/*",
			},
		});

		expect(response.status()).toBe(200);
		const scriptContent = await response.text();
		expect(scriptContent).toContain("#!/bin/sh");
	});

	test("script checks for required commands", async ({ request }) => {
		const response = await request.get("/install.sh");
		const scriptContent = await response.text();

		// Verify preflight checks for required commands
		expect(scriptContent).toContain("command -v");
		expect(scriptContent).toContain("curl");
		expect(scriptContent).toContain("mktemp");
		expect(scriptContent).toContain("grep");
		expect(scriptContent).toContain("sed");
		expect(scriptContent).toContain("uname");
		expect(scriptContent).toContain("chmod");
	});
});
