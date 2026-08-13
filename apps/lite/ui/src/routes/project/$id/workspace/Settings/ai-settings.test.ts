import { describe, expect, it, vi } from "vitest";
import { configurationUpdate, modelSelection, saveThenTest } from "./ai-settings.ts";
import type { AiConfiguration, AiConfigurationUpdate } from "@gitbutler/but-sdk";

describe("AI settings", () => {
	it("uses custom for models outside the presets", () => {
		expect(modelSelection("gpt-5.4-nano", ["gpt-5.4-nano"])).toBe("gpt-5.4-nano");
		expect(modelSelection("local-model", ["gpt-5.4-nano"])).toBe("custom");
	});

	it("offers a supported provider when OpenRouter is configured elsewhere", () => {
		expect(configurationUpdate({ provider: "openrouter" } as AiConfiguration).provider).toBe(
			"openai",
		);
	});

	it("saves before starting the connection test", async () => {
		const order: Array<string> = [];
		const configuration = {} as AiConfiguration;
		const update = {} as AiConfigurationUpdate;
		vi.stubGlobal("window", {
			lite: {
				updateAiConfiguration: vi.fn(async () => {
					order.push("save");
					return configuration;
				}),
				streamAiResponse: vi.fn(async () => {
					order.push("stream");
					return "ok";
				}),
			},
		});

		try {
			await saveThenTest(update, () => order.push("saved"), vi.fn());
			expect(order).toEqual(["save", "saved", "stream"]);
		} finally {
			vi.unstubAllGlobals();
		}
	});
});
