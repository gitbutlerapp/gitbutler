import { listOpenAIModels } from "$lib/ai/openAIClient";
import { afterEach, describe, expect, test, vi } from "vitest";

describe("listOpenAIModels", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	test("requests and returns the models exposed by a custom endpoint", async () => {
		const fetchMock = vi.fn().mockResolvedValue(
			new Response(
				JSON.stringify({
					object: "list",
					data: [
						{ id: "z-model", object: "model", created: 0, owned_by: "test" },
						{ id: "a-model", object: "model", created: 0, owned_by: "test" },
						{ id: "z-model", object: "model", created: 0, owned_by: "test" },
					],
				}),
				{ headers: { "content-type": "application/json" } },
			),
		);
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			listOpenAIModels("test-api-key", "https://models.example.test/v1"),
		).resolves.toEqual(["a-model", "z-model"]);

		expect(fetchMock).toHaveBeenCalledOnce();
		const [input, init] = fetchMock.mock.calls[0] as [RequestInfo | URL, RequestInit | undefined];
		const requestUrl =
			typeof input === "string" ? input : "url" in input ? input.url : input.toString();
		const headers = new Headers(init?.headers);
		expect(init?.method?.toUpperCase()).toBe("GET");
		expect(requestUrl).toBe("https://models.example.test/v1/models");
		expect(headers.get("authorization")).toBe("Bearer test-api-key");
	});

	test("classifies authentication failures without exposing the response body", async () => {
		const fetchMock = vi.fn().mockResolvedValue(
			new Response(JSON.stringify({ error: { message: "sensitive upstream detail" } }), {
				status: 401,
				headers: { "content-type": "application/json" },
			}),
		);
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			listOpenAIModels("test-api-key", "https://models.example.test/v1"),
		).rejects.toMatchObject({
			kind: "authentication",
			message: "Model discovery failed: authentication",
		});
	});

	test("classifies an unsupported models endpoint", async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 404 }));
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			listOpenAIModels("test-api-key", "https://models.example.test/v1"),
		).rejects.toMatchObject({ kind: "unsupported" });
	});

	test("classifies an unavailable endpoint", async () => {
		const fetchMock = vi.fn().mockRejectedValue(new TypeError("fetch failed"));
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			listOpenAIModels("test-api-key", "https://models.example.test/v1"),
		).rejects.toMatchObject({ kind: "unavailable" });
	});

	test("classifies a malformed model list", async () => {
		const fetchMock = vi.fn().mockResolvedValue(
			new Response(JSON.stringify({ object: "list", data: [{ object: "model" }] }), {
				headers: { "content-type": "application/json" },
			}),
		);
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			listOpenAIModels("test-api-key", "https://models.example.test/v1"),
		).rejects.toMatchObject({ kind: "invalid-response" });
	});
});
