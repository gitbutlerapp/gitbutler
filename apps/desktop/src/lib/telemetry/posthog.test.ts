import { EventContext } from "$lib/telemetry/eventContext";
import { OnboardingEvent, PostHogWrapper, sampleEvent } from "$lib/telemetry/posthog";
import { describe, expect, test, vi } from "vitest";
import type { IBackend } from "$lib/backend";
import type { SettingsService } from "$lib/settings/appSettings";

function createPostHogWrapper() {
	const capture = vi.fn();
	const wrapper = new PostHogWrapper({} as SettingsService, {} as IBackend, new EventContext());
	Reflect.set(wrapper, "_instance", { capture });
	return { capture, wrapper };
}

describe("captureOnboarding", () => {
	test("captures the parsed set-project-active error", () => {
		const { capture, wrapper } = createPostHogWrapper();
		const error = {
			name: "Project activation failed",
			message: "The project could not be opened",
			code: "ProjectUnavailable",
		};

		wrapper.captureOnboarding(OnboardingEvent.SetProjectActiveFailed, error);

		expect(capture).toHaveBeenCalledWith(OnboardingEvent.SetProjectActiveFailed, {
			error_title: error.name,
			error_message: error.message,
			error_code: error.code,
		});
	});

	test("does not fabricate error fields for a successful activation", () => {
		const { capture, wrapper } = createPostHogWrapper();

		wrapper.captureOnboarding(OnboardingEvent.SetProjectActive);

		expect(capture).toHaveBeenCalledWith(OnboardingEvent.SetProjectActive, {});
	});
});

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
