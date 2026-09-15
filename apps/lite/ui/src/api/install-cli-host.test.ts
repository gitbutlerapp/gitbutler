import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { installCli } from "#electron/cli.ts";
import { exposedEndpoints } from "#electron/ipc.ts";

const { app, installCliV2 } = vi.hoisted(() => ({
	app: { isPackaged: true, isInApplicationsFolder: vi.fn(() => true) },
	installCliV2: vi.fn<(...args: Array<unknown>) => Promise<boolean>>(),
}));
vi.mock("electron", () => ({ app }));
vi.mock("@gitbutler/but-sdk", () => ({ installCliV2 }));

beforeEach(() => {
	vi.stubGlobal("process", {
		...process,
		platform: "darwin",
		resourcesPath: "/Applications/GitButler Next.app/Contents/Resources",
	});
	app.isPackaged = true;
	app.isInApplicationsFolder.mockReturnValue(true);
	installCliV2.mockReset().mockResolvedValue(true);
});
afterEach(() => vi.unstubAllGlobals());

it("resolves source in main, without exposing the source-path endpoint", async () => {
	expect(exposedEndpoints).not.toContain("installCliV2");
	await expect(installCli()).resolves.toBe(true);
	expect(installCliV2).toHaveBeenCalledWith(
		"/Applications/GitButler Next.app/Contents/Resources/bin/but",
		"Refuse",
	);
});

it("preserves cancellation and installation failures", async () => {
	installCliV2.mockResolvedValueOnce(false);
	await expect(installCli()).resolves.toBe(false);
	installCliV2.mockRejectedValueOnce(new Error("Destination exists"));
	await expect(installCli()).rejects.toThrow("Destination exists");
});

it("rejects non-macOS, unpackaged and uninstalled apps before calling Rust", async () => {
	vi.stubGlobal("process", { ...process, platform: "linux" });
	await expect(installCli()).rejects.toThrow("packaged macOS app");
	vi.stubGlobal("process", { ...process, platform: "darwin" });
	app.isPackaged = false;
	await expect(installCli()).rejects.toThrow("packaged macOS app");
	app.isPackaged = true;
	app.isInApplicationsFolder.mockReturnValue(false);
	await expect(installCli()).rejects.toThrow("Move GitButler to Applications");
	expect(installCliV2).not.toHaveBeenCalled();
});
