import { sampleEvent } from "$lib/telemetry/posthog";
import { describe, expect, test } from "vitest";

describe("sampleEvent", () => {
	test("stamps samplingRate on emitted sampled events", () => {
		expect(sampleEvent("tauri_command", { command: "head_info", failure: false }, 0.01)).toEqual({
			command: "head_info",
			failure: false,
			samplingRate: 0.05,
		});
	});

	test("drops successful sampled events above the rate", () => {
		expect(sampleEvent("tauri_command", { command: "head_info", failure: false }, 0.5)).toBeNull();
		expect(
			sampleEvent(
				"tauri_command",
				{ command: "workspace_fetch_from_remotes", failure: false },
				0.5,
			),
		).toBeNull();
	});

	test("samples failures like successes", () => {
		expect(
			sampleEvent(
				"tauri_command",
				{ command: "workspace_fetch_from_remotes", failure: true },
				0.01,
			),
		).toEqual({ command: "workspace_fetch_from_remotes", failure: true, samplingRate: 0.5 });
		expect(
			sampleEvent(
				"tauri_command",
				{ command: "workspace_fetch_from_remotes", failure: true },
				0.99,
			),
		).toBeNull();
	});

	test("leaves unsampled events untouched", () => {
		expect(sampleEvent("tauri_command", { command: "stack_details" }, 0.99)).toEqual({
			command: "stack_details",
		});
		expect(sampleEvent("some_event", undefined, 0.99)).toEqual({});
	});
});
