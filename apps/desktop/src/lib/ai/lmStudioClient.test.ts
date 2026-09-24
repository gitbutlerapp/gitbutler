import { LMStudioClient } from "$lib/ai/lmStudioClient";
import { MessageRole, type Prompt } from "$lib/ai/types";
import { afterEach, describe, expect, test, vi } from "vitest";

const prompt: Prompt = [{ role: MessageRole.User, content: "Hello" }];

function stubFetch() {
	const fetchMock = vi.fn<typeof fetch>(
		async () => new Response(JSON.stringify({ choices: [{ message: { content: "Hi" } }] })),
	);
	vi.stubGlobal("fetch", fetchMock);
	return fetchMock;
}

function requestBody(fetchMock: ReturnType<typeof stubFetch>) {
	return JSON.parse(fetchMock.mock.calls[0]?.[1]?.body as string);
}

describe("LMStudioClient", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	test("omits max_tokens when no limit is given", async () => {
		const fetchMock = stubFetch();
		const client = new LMStudioClient("http://localhost:1234", "model");

		expect(await client.evaluate(prompt)).toBe("Hi");
		expect(requestBody(fetchMock)).not.toHaveProperty("max_tokens");
	});

	test("sends an explicit max_tokens limit", async () => {
		const fetchMock = stubFetch();
		const client = new LMStudioClient("http://localhost:1234", "model");

		await client.evaluate(prompt, { maxTokens: 256 });
		expect(requestBody(fetchMock).max_tokens).toBe(256);
	});
});
