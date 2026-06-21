export type CodingAgent = "none" | "codex" | "claude";

export type ProjectSettings = {
	checkpointDebounceMs: number;
	codingAgent: CodingAgent;
	fetchIntervalMs: number;
};

export const defaultProjectSettings: ProjectSettings = {
	checkpointDebounceMs: 10_000,
	codingAgent: "none",
	fetchIntervalMs: 15 * 60 * 1000,
};

export const minCheckpointDebounceMs = 1_000;
export const maxCheckpointDebounceMs = 60_000;
export const allowedFetchIntervalMs = [
	0,
	5 * 60 * 1000,
	15 * 60 * 1000,
	30 * 60 * 1000,
	60 * 60 * 1000,
];

export function normalizeCheckpointDebounceMs(value: unknown): number {
	const parsed = typeof value === "number" ? value : Number(value);
	if (!Number.isFinite(parsed)) return defaultProjectSettings.checkpointDebounceMs;

	const rounded = Math.round(parsed);
	if (rounded < minCheckpointDebounceMs || rounded > maxCheckpointDebounceMs)
		return defaultProjectSettings.checkpointDebounceMs;
	return rounded;
}

export function normalizeCheckpointDebounceMsWithFallback(
	value: unknown,
	fallback: number,
): number {
	const parsed = typeof value === "number" ? value : Number(value);
	if (!Number.isFinite(parsed)) return normalizeCheckpointDebounceMs(fallback);

	const rounded = Math.round(parsed);
	if (rounded < minCheckpointDebounceMs || rounded > maxCheckpointDebounceMs)
		return normalizeCheckpointDebounceMs(fallback);
	return rounded;
}

export function normalizeCodingAgent(value: unknown): CodingAgent {
	if (value === "codex" || value === "claude" || value === "none") return value;
	return defaultProjectSettings.codingAgent;
}

export function normalizeFetchIntervalMs(value: unknown): number {
	const parsed = typeof value === "number" ? value : Number(value);
	if (!Number.isFinite(parsed)) return defaultProjectSettings.fetchIntervalMs;
	const rounded = Math.round(parsed);
	return allowedFetchIntervalMs.includes(rounded)
		? rounded
		: defaultProjectSettings.fetchIntervalMs;
}

export function normalizeFetchIntervalMsWithFallback(value: unknown, fallback: number): number {
	const parsed = typeof value === "number" ? value : Number(value);
	if (!Number.isFinite(parsed)) return normalizeFetchIntervalMs(fallback);
	const rounded = Math.round(parsed);
	return allowedFetchIntervalMs.includes(rounded) ? rounded : normalizeFetchIntervalMs(fallback);
}

export function normalizeProjectSettings(value: Partial<ProjectSettings>): ProjectSettings {
	return {
		checkpointDebounceMs: normalizeCheckpointDebounceMs(value.checkpointDebounceMs),
		codingAgent: normalizeCodingAgent(value.codingAgent),
		fetchIntervalMs: normalizeFetchIntervalMs(value.fetchIntervalMs),
	};
}
