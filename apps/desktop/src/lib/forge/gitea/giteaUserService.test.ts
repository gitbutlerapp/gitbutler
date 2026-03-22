import {
	giteaAccountIdentifierToString,
	isSameGiteaAccountIdentifier,
	stringToGiteaAccountIdentifier,
} from "$lib/forge/gitea/giteaUserService.svelte";
import { describe, expect, test } from "vitest";

describe("giteaUserService helpers", () => {
	test("round-trips account identifiers", () => {
		const account = {
			host: "https://codeberg.org",
			username: "alice",
		};

		expect(stringToGiteaAccountIdentifier(giteaAccountIdentifierToString(account))).toEqual(
			account,
		);
	});

	test("rejects malformed serialized identifiers", () => {
		expect(stringToGiteaAccountIdentifier("not-a-valid-account")).toBeNull();
	});

	test("compares host and username", () => {
		expect(
			isSameGiteaAccountIdentifier(
				{ host: "https://codeberg.org", username: "alice" },
				{ host: "https://codeberg.org", username: "alice" },
			),
		).toBe(true);
		expect(
			isSameGiteaAccountIdentifier(
				{ host: "https://codeberg.org", username: "alice" },
				{ host: "https://codeberg.org", username: "bob" },
			),
		).toBe(false);
	});
});
