import { streamGeneratedText } from "#ui/ai-streaming.ts";
import { describe, expect, it, vi } from "vitest";

describe("streamGeneratedText", () => {
	it("leaves the field alone when the stream fails before writing anything", async () => {
		const onValue = vi.fn();
		const onFailure = vi.fn();
		await expect(
			streamGeneratedText(
				async () => {
					throw new Error("failed before response");
				},
				onValue,
				onFailure,
			),
		).rejects.toThrow("failed before response");

		expect(onValue).not.toHaveBeenCalled();
		expect(onFailure).not.toHaveBeenCalled();
	});

	it("asks for a restore once a partial answer has overwritten the field", async () => {
		const onValue = vi.fn();
		const onFailure = vi.fn();
		await expect(
			streamGeneratedText(
				async (onToken) => {
					onToken("partial");
					throw new Error("failed during response");
				},
				onValue,
				onFailure,
			),
		).rejects.toThrow("failed during response");

		expect(onValue.mock.calls).toEqual([["partial"]]);
		expect(onFailure).toHaveBeenCalledOnce();
	});

	it("streams each partial, then the whole response", async () => {
		const onValue = vi.fn();
		const onFailure = vi.fn();
		await streamGeneratedText(
			async (onToken) => {
				onToken("feat: ");
				onToken("generated");
				return "feat: generated";
			},
			onValue,
			onFailure,
		);

		expect(onValue.mock.calls).toEqual([["feat: "], ["feat: generated"], ["feat: generated"]]);
		expect(onFailure).not.toHaveBeenCalled();
	});
});
