import { vi } from "vitest";

export function getSettingsdServiceMock() {
	const SettingsServiceMock = vi.fn();

	SettingsServiceMock.prototype.updateTelemetryDistinctId = vi.fn();

	return SettingsServiceMock;
}
