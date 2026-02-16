import { TokenMemoryService } from "$lib/stores/tokenMemoryService";
import { persisted } from "@gitbutler/shared/persisted";
import { get } from "svelte/store";
import { test, describe, expect } from "vitest";

describe("TokenMemoryService", () => {
	test("It gets the value form secret service", async () => {
		const tokenMemoryService = new TokenMemoryService();
		tokenMemoryService.setToken("foobar");

		const token = await new Promise<string>((resolve) => {
			tokenMemoryService.token.subscribe((token) => {
				if (token) {
					resolve(token);
				}
			});
		});

		expect(token).eq("foobar");
	});

	test("Setting the token to be different", async () => {
		const tokenMemoryService = new TokenMemoryService();
		tokenMemoryService.setToken("foobar");

		const token = await new Promise<string>((resolve) => {
			tokenMemoryService.token.subscribe((token) => {
				if (token) {
					resolve(token);
				}
			});
		});

		expect(token).eq("foobar");

		await tokenMemoryService.setToken("anotherThing");

		expect(get(tokenMemoryService.token)).eq("anotherThing");
	});

	test("Setting the token to be undefined (logging out)", async () => {
		const tokenMemoryService = new TokenMemoryService();
		tokenMemoryService.setToken("foobar");

		const token = await new Promise<string>((resolve) => {
			tokenMemoryService.token.subscribe((token) => {
				if (token) {
					resolve(token);
				}
			});
		});

		expect(token).eq("foobar");

		await tokenMemoryService.setToken(undefined);

		expect(get(tokenMemoryService.token)).eq(undefined);
	});

	test("Old tokens in localStorage are ignored", async () => {
		const tokenKey = "TokenMemoryService-authToken";
		const oldToken = persisted<string | undefined>(undefined, tokenKey);
		oldToken.set("foobar");

		const tokenMemoryService = new TokenMemoryService();
		tokenMemoryService.setToken("not-foobar");

		expect(get(tokenMemoryService.token)).eq("not-foobar");
	});
});
