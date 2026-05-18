import { ModelKind } from "$lib/ai/types";

export const DEFAULT_AI_CREDENTIAL_CHECK_TIMEOUT_MS = 20_000;
export const ACP_AI_CREDENTIAL_CHECK_TIMEOUT_MS = 60_000;

export function getAiCredentialCheckTimeoutMs(modelKind: ModelKind | undefined) {
	return modelKind === ModelKind.ACP
		? ACP_AI_CREDENTIAL_CHECK_TIMEOUT_MS
		: DEFAULT_AI_CREDENTIAL_CHECK_TIMEOUT_MS;
}
