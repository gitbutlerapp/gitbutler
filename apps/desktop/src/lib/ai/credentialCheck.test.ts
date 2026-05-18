import {
	ACP_AI_CREDENTIAL_CHECK_TIMEOUT_MS,
	DEFAULT_AI_CREDENTIAL_CHECK_TIMEOUT_MS,
	getAiCredentialCheckTimeoutMs,
} from "$lib/ai/credentialCheck";
import { ModelKind } from "$lib/ai/types";
import { describe, expect, test } from "vitest";

describe("getAiCredentialCheckTimeoutMs", () => {
	test("uses the default timeout for non-ACP providers", () => {
		expect(getAiCredentialCheckTimeoutMs(ModelKind.OpenAI)).toBe(
			DEFAULT_AI_CREDENTIAL_CHECK_TIMEOUT_MS,
		);
	});

	test("allows slower ACP agent startup and responses", () => {
		expect(getAiCredentialCheckTimeoutMs(ModelKind.ACP)).toBe(ACP_AI_CREDENTIAL_CHECK_TIMEOUT_MS);
		expect(getAiCredentialCheckTimeoutMs(ModelKind.ACP)).toBeGreaterThan(25_500);
	});
});
