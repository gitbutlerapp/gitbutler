import { initUserSettings, type UiState } from "$lib/state/uiState.svelte";
import type { TerminalService } from "$lib/settings/terminalService";
import type { TerminalSettings } from "$lib/state/uiState.svelte";
import { describe, expect, test, vi, beforeEach } from "vitest";

const LEGACY_KEY = "settings-json";

type MockProp = { current: unknown; set: ReturnType<typeof vi.fn> };

/** Creates a mock UiState with trackable `.set()` calls on each global property. */
function mockUiState(overrides?: { defaultTerminal?: unknown }) {
	const global = {
		zoom: { current: 1, set: vi.fn() },
		theme: { current: "system", set: vi.fn() },
		tabSize: { current: 4, set: vi.fn() },
		diffFont: { current: "Geist Mono, Menlo, monospace", set: vi.fn() },
		defaultCodeEditor: {
			current: { schemeIdentifer: "vscode", displayName: "VSCode" },
			set: vi.fn(),
		},
		defaultTerminal: {
			current: overrides?.defaultTerminal ?? {
				identifier: "terminal",
				displayName: "Terminal",
				platform: "macos",
			},
			set: vi.fn(),
		},
		wrapText: { current: false, set: vi.fn() },
		diffLigatures: { current: false, set: vi.fn() },
		pathFirst: { current: true, set: vi.fn() },
	} satisfies Record<string, MockProp>;

	// initUserSettings only accesses .global and its properties, so the
	// narrow cast here is safe — we control every field the function touches.
	const uiState = { global } as unknown as UiState;
	return { uiState, global };
}

/** Creates a mock TerminalService that returns the first recommended terminal for each platform. */
function mockTerminalService(): TerminalService {
	const platformTerminals: Record<string, TerminalSettings | null> = {
		macos: { identifier: "terminal", displayName: "Terminal", platform: "macos" },
		windows: { identifier: "powershell", displayName: "PowerShell", platform: "windows" },
		linux: { identifier: "gnome-terminal", displayName: "GNOME Terminal", platform: "linux" },
	};

	return {
		getTerminalOptionsForPlatform: vi.fn(),
		getRecommendedTerminalForPlatform: vi.fn((platform: string) =>
			Promise.resolve(platformTerminals[platform] ?? null),
		),
	} as unknown as TerminalService;
}

beforeEach(() => {
	localStorage.clear();
});

describe("initUserSettings", () => {
	test("migrates legacy settings into UiState", () => {
		localStorage.setItem(LEGACY_KEY, JSON.stringify({ zoom: 1.5, tabSize: 2, wrapText: true }));

		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.zoom.set).toHaveBeenCalledWith(1.5);
		expect(global.tabSize.set).toHaveBeenCalledWith(2);
		expect(global.wrapText.set).toHaveBeenCalledWith(true);
	});

	test("removes the legacy key after migration", () => {
		localStorage.setItem(LEGACY_KEY, JSON.stringify({ zoom: 2 }));

		const { uiState } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
	});

	test("does nothing when no legacy data exists", () => {
		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.zoom.set).not.toHaveBeenCalled();
		expect(global.tabSize.set).not.toHaveBeenCalled();
	});

	test("removes corrupted legacy data without crashing", () => {
		localStorage.setItem(LEGACY_KEY, "not valid json{{{");

		const { uiState } = mockUiState();
		expect(() => initUserSettings(uiState, "macos")).not.toThrow();
		expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
	});

	test("skips keys not present on UiState.global (prototype pollution guard)", () => {
		// Use a raw JSON string because JSON.stringify({ __proto__: ... }) won't
		// produce an own "__proto__" key in the output.
		localStorage.setItem(
			LEGACY_KEY,
			'{"__proto__":{"malicious":true},"toString":"bad","zoom":1.5}',
		);

		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.zoom.set).toHaveBeenCalledWith(1.5);
		expect(global).not.toHaveProperty("__proto__.set");
	});

	test("skips null values from legacy data", () => {
		// JSON.stringify drops undefined values, so we test with null instead
		// which is what JSON can actually represent.
		localStorage.setItem(LEGACY_KEY, JSON.stringify({ zoom: null, tabSize: 8 }));

		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.zoom.set).not.toHaveBeenCalled();
		expect(global.tabSize.set).toHaveBeenCalledWith(8);
	});

	test("corrects terminal default on Windows", async () => {
		const { uiState, global } = mockUiState();
		const terminalService = mockTerminalService();
		await initUserSettings(uiState, "windows", terminalService);

		expect(global.defaultTerminal.set).toHaveBeenCalledWith(
			expect.objectContaining({ identifier: "powershell", platform: "windows" }),
		);
	});

	test("corrects terminal default on Linux", async () => {
		const { uiState, global } = mockUiState();
		const terminalService = mockTerminalService();
		await initUserSettings(uiState, "linux", terminalService);

		expect(global.defaultTerminal.set).toHaveBeenCalledWith(
			expect.objectContaining({ identifier: "gnome-terminal", platform: "linux" }),
		);
	});

	test("does not overwrite terminal when platform already matches", () => {
		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.defaultTerminal.set).not.toHaveBeenCalled();
	});

	test("migrates nested object values like defaultCodeEditor", () => {
		const editor = { schemeIdentifer: "cursor", displayName: "Cursor" };
		localStorage.setItem(LEGACY_KEY, JSON.stringify({ defaultCodeEditor: editor }));

		const { uiState, global } = mockUiState();
		initUserSettings(uiState, "macos");

		expect(global.defaultCodeEditor.set).toHaveBeenCalledWith(editor);
	});
});
