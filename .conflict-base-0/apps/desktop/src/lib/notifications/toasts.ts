import posthog from "posthog-js";
import { writable, type Writable } from "svelte/store";
import type { MessageStyle } from "@gitbutler/ui";

type ExtraAction = {
	label: string;
	testId?: string;
	onClick: (dismiss: () => void) => void;
};

export interface Toast {
	id?: string;
	testId?: string;
	message?: string;
	error?: any;
	title?: string;
	style?: MessageStyle;
	extraAction?: ExtraAction;
}

export const toastStore: Writable<Toast[]> = writable([]);

let idCounter = 0;

const TOAST_CAPTURE_LIMIT = 60;
const TOAST_CAPTURE_WINDOW_MS = 60 * 60 * 1000; // 1 hour
const toastCaptureTimestamps: number[] = [];

/**
 * Per-renderer rate limit (60/hour) shared across all toast telemetry —
 * `toast:show_error` from `showError`, `toast:show_warning` from
 * `showWarning`. Used so a runaway error loop can't flood PostHog or
 * Sentry from a single user.
 */
export function shouldCaptureToast(): boolean {
	const now = Date.now();
	const cutoff = now - TOAST_CAPTURE_WINDOW_MS;
	while (toastCaptureTimestamps.length > 0 && toastCaptureTimestamps[0]! <= cutoff) {
		toastCaptureTimestamps.shift();
	}
	if (toastCaptureTimestamps.length >= TOAST_CAPTURE_LIMIT) {
		return false;
	}
	toastCaptureTimestamps.push(now);
	return true;
}

export function showToast(toast: Toast) {
	// `toast:show_error` and `toast:show_warning` are captured by
	// `showError` and `showWarning` respectively — telemetry lives next
	// to the semantic call, not in this low-level UI primitive. Callers
	// of `showToast` directly don't emit telemetry; convert them to
	// `showWarning` / `showError` if telemetry is wanted.
	toast.message = toast.message?.replace(/^ */gm, "");
	if (!toast.id) {
		toast = { ...toast, id: `${idCounter++}` };
	}
	toastStore.update((items) => [
		...items.filter((t) => toast.id === undefined || t.id !== toast.id),
		toast,
	]);
}

export function showInfo(title: string, message: string, extraAction?: ExtraAction) {
	showToast({ title, message, style: "info", extraAction });
}

export function showWarning(
	title: string,
	message: string,
	extraAction?: ExtraAction,
	testId?: string,
) {
	if (shouldCaptureToast()) {
		posthog.capture("toast:show_warning", {
			warning_test_id: testId,
			warning_title: title,
			warning_message: message,
		});
	}
	showToast({ title, message, style: "warning", extraAction, testId });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id !== messageId));
}
