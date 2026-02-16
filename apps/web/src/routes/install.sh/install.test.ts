import installScript from "$scripts/install.sh?raw";
import { describe, it, expect } from "vitest";

describe("Install script import", () => {
	it("successfully imports the install script", () => {
		expect(installScript).toBeDefined();
		expect(typeof installScript).toBe("string");
		expect(installScript.length).toBeGreaterThan(0);
	});

	it("contains shell shebang", () => {
		expect(installScript).toContain("#!/bin/sh");
	});

	it("is a bootstrap script that downloads the installer binary", () => {
		// Verify this is the lightweight bootstrap
		expect(installScript).toContain("GitButler installer bootstrap script");
		expect(installScript).toContain("https://app.gitbutler.com/installers/info");
		expect(installScript).toContain("https://releases.gitbutler.com");
	});

	it("has proper error handling", () => {
		expect(installScript).toContain("set -e");
	});

	it("detects OS and architecture", () => {
		expect(installScript).toContain("uname -s");
		expect(installScript).toContain("uname -m");
		expect(installScript).toContain("darwin");
		expect(installScript).toContain("x86_64");
		expect(installScript).toContain("aarch64");
	});

	it("validates download URLs", () => {
		// Verify URL validation exists
		expect(installScript).toContain("EFFECTIVE_URL");
		expect(installScript).toContain("untrusted URL");
	});

	it("forwards arguments to installer binary", () => {
		// Verify args are forwarded (supports nightly, versions, etc.)
		expect(installScript).toContain('exec "$INSTALLER_BIN" "$@"');
	});

	it("checks for required commands", () => {
		// Verify preflight checks exist
		expect(installScript).toContain("command -v");
		expect(installScript).toContain("curl");
		expect(installScript).toContain("mktemp");
	});
});
