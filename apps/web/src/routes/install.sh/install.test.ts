import { GET } from "./+server";
import installScript from "$scripts/install.sh?raw";
import { afterEach, beforeEach, describe, it, expect, vi } from "vitest";

function requestEvent(host: string, headers: Record<string, string> = {}) {
	const url = new URL(`https://${host}/install.sh`);
	return {
		request: new Request(url, { headers: { "user-agent": "curl/8.4.0", ...headers } }),
		url,
	} as unknown as Parameters<typeof GET>[0];
}

describe("GET /install.sh", () => {
	let fetchMock: ReturnType<typeof vi.fn>;

	// Stub fetch before every test so no test can ever send a real event to
	// production PostHog (Node's real fetch is live in this environment).
	beforeEach(() => {
		fetchMock = vi.fn().mockResolvedValue(new Response(null));
		vi.stubGlobal("fetch", fetchMock);
		vi.spyOn(console, "error").mockImplementation(() => {});
	});

	afterEach(() => {
		vi.unstubAllGlobals();
		vi.restoreAllMocks();
	});

	it("serves the script even when the capture request fails", async () => {
		fetchMock.mockRejectedValue(new Error("posthog down"));
		const response = await GET(requestEvent("gitbutler.com"));
		expect(response.status).toBe(200);
		expect(response.headers.get("Content-Type")).toBe("text/plain; charset=utf-8");
		expect(await response.text()).toBe(installScript);
		expect(console.error).toHaveBeenCalled();
	});

	it("captures a fetch event on the production host", async () => {
		await GET(requestEvent("gitbutler.com", { "x-vercel-forwarded-for": "203.0.113.7" }));
		expect(fetchMock).toHaveBeenCalledOnce();
		const [target, init] = fetchMock.mock.calls[0];
		expect(target).toBe("https://eu.i.posthog.com/capture/");
		const body = JSON.parse(init.body);
		expect(body.api_key).toBeTruthy();
		expect(body.event).toBe("install_script_fetched");
		expect(body.properties.$process_person_profile).toBe(false);
		expect(body.properties.$raw_user_agent).toBe("curl/8.4.0");
		expect(body.properties.$ip).toBe("203.0.113.7");
	});

	it("takes the peer address from a client-forged x-forwarded-for chain", async () => {
		await GET(requestEvent("gitbutler.com", { "x-forwarded-for": "1.2.3.4, 203.0.113.7" }));
		const body = JSON.parse(fetchMock.mock.calls[0][1].body);
		expect(body.properties.$ip).toBe("203.0.113.7");
	});

	it("omits $ip for non-IP header values", async () => {
		await GET(requestEvent("gitbutler.com", { "x-forwarded-for": "unknown" }));
		const body = JSON.parse(fetchMock.mock.calls[0][1].body);
		expect(body.properties).not.toHaveProperty("$ip");
	});

	it("omits $ip when no forwarding header exists", async () => {
		await GET(requestEvent("gitbutler.com"));
		const body = JSON.parse(fetchMock.mock.calls[0][1].body);
		expect(body.properties).not.toHaveProperty("$ip");
	});

	it("does not capture on non-production hosts", async () => {
		await GET(requestEvent("localhost"));
		expect(fetchMock).not.toHaveBeenCalled();
	});
});

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
