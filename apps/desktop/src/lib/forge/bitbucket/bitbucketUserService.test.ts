import {
	bitbucketAccountIdentifierToString,
	isSameBitbucketAccountIdentifier,
	stringToBitbucketAccountIdentifier,
} from "$lib/forge/bitbucket/bitbucketUserService.svelte";
import { describe, expect, test } from "vitest";
import type { BitbucketAccountIdentifier } from "@gitbutler/but-sdk";

const UNIT_SEP = "\u001F";

describe("bitbucket account identifier serialization", () => {
	test("serializes an apiToken account", () => {
		const account: BitbucketAccountIdentifier = {
			type: "apiToken",
			info: {
				email: "alice@example.com",
			},
		};

		expect(bitbucketAccountIdentifierToString(account)).toBe(
			`apiToken${UNIT_SEP}alice@example.com`,
		);
	});
});

describe("bitbucket account identifier deserialization", () => {
	test("deserializes an apiToken account", () => {
		expect(
			stringToBitbucketAccountIdentifier(`apiToken${UNIT_SEP}alice@example.com`),
		).toStrictEqual({
			type: "apiToken",
			info: {
				email: "alice@example.com",
			},
		});
	});

	test("returns null for malformed or unsupported values", () => {
		expect(stringToBitbucketAccountIdentifier("apiToken")).toBeNull();
		expect(stringToBitbucketAccountIdentifier(`enterprise${UNIT_SEP}alice@example.com`)).toBeNull();
		expect(stringToBitbucketAccountIdentifier("")).toBeNull();
	});
});

describe("bitbucket account identifier round-trip", () => {
	test("round-trips supported account types", () => {
		const account: BitbucketAccountIdentifier = {
			type: "apiToken",
			info: {
				email: "alice@example.com",
			},
		};

		const serialized = bitbucketAccountIdentifierToString(account);
		expect(stringToBitbucketAccountIdentifier(serialized)).toStrictEqual(account);
	});
});

describe("isSameBitbucketAccountIdentifier", () => {
	test("matches accounts with the same email", () => {
		const a: BitbucketAccountIdentifier = { type: "apiToken", info: { email: "a@example.com" } };
		const b: BitbucketAccountIdentifier = { type: "apiToken", info: { email: "a@example.com" } };
		expect(isSameBitbucketAccountIdentifier(a, b)).toBe(true);
	});

	test("does not match accounts with different emails", () => {
		const a: BitbucketAccountIdentifier = { type: "apiToken", info: { email: "a@example.com" } };
		const b: BitbucketAccountIdentifier = { type: "apiToken", info: { email: "b@example.com" } };
		expect(isSameBitbucketAccountIdentifier(a, b)).toBe(false);
	});
});
