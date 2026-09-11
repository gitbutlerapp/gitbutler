import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
	init: vi.fn(),
	register: vi.fn(),
	settings: vi.fn(),
	isPackaged: vi.fn(),
}));

vi.mock("posthog-js/dist/module.no-external", () => ({
	default: { init: mocks.init, register: mocks.register },
}));
vi.mock("posthog-js/dist/exception-autocapture", () => ({}));
vi.mock("#ui/api/queries.ts", () => ({
	appSettingsQueryOptions: { queryKey: ["settings"], queryFn: mocks.settings },
	isPackagedQueryOptions: { queryKey: ["isPackaged"], queryFn: mocks.isPackaged },
	versionQueryOptions: { queryKey: ["version"], queryFn: async () => "1.2.3" },
	userProfileQueryOptions: { queryKey: ["profile"], queryFn: async () => null },
}));

beforeEach(() => {
	vi.clearAllMocks();
	vi.stubEnv("DEV", false);
	// Vitest converts process.env defines to mutable env values, so this overrides the Vite default.
	vi.stubEnv("CHANNEL", "nightly");
	mocks.isPackaged.mockResolvedValue(true);
	mocks.settings.mockResolvedValue({
		telemetry: { appErrorReportingEnabled: true, appDistinctId: "install-id" },
	});
});

afterEach(() => vi.unstubAllEnvs());

test.each(["nightly", "release"])(
	"does not initialize a production %s UI in unpackaged Electron",
	async (channel) => {
		vi.stubEnv("CHANNEL", channel);
		mocks.isPackaged.mockResolvedValue(false);
		const { initErrorReporting } = await import("./error-reporting.ts");
		await initErrorReporting(new QueryClient());

		expect(mocks.settings).not.toHaveBeenCalled();
		expect(mocks.init).not.toHaveBeenCalled();
	},
);

test.each([undefined, "", "dev", "unexpected"])(
	"does not initialize reporting for packaged channel %s",
	async (channel) => {
		vi.stubEnv("CHANNEL", channel);
		const { initErrorReporting } = await import("./error-reporting.ts");
		await initErrorReporting(new QueryClient());

		expect(mocks.settings).not.toHaveBeenCalled();
		expect(mocks.init).not.toHaveBeenCalled();
	},
);

test("does not initialize reporting in the dev server even with a nightly channel", async () => {
	vi.stubEnv("DEV", true);
	const { initErrorReporting } = await import("./error-reporting.ts");
	await initErrorReporting(new QueryClient());

	expect(mocks.settings).not.toHaveBeenCalled();
	expect(mocks.init).not.toHaveBeenCalled();
});

test.each(["nightly", "release"])("keeps exception reporting for %s", async (channel) => {
	vi.stubEnv("CHANNEL", channel);
	const { initErrorReporting } = await import("./error-reporting.ts");
	await initErrorReporting(new QueryClient());

	expect(mocks.init).toHaveBeenCalledWith(
		expect.any(String),
		expect.objectContaining({
			bootstrap: { distinctID: "install-id" },
			capture_exceptions: {
				capture_unhandled_errors: true,
				capture_unhandled_rejections: true,
				capture_console_errors: false,
			},
		}),
	);
	expect(mocks.register).toHaveBeenCalledWith({
		appName: "gitbutler-next",
		appVersion: "1.2.3",
		appChannel: channel,
		container: "electron",
		process: "renderer",
		environment: "production",
	});
});

test("respects disabled error reporting in a nightly build", async () => {
	mocks.settings.mockResolvedValue({ telemetry: { appErrorReportingEnabled: false } });
	const { initErrorReporting } = await import("./error-reporting.ts");
	await initErrorReporting(new QueryClient());

	expect(mocks.settings).toHaveBeenCalledOnce();
	expect(mocks.init).not.toHaveBeenCalled();
});
