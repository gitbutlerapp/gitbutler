import {
	githubAccountIdentifierToString,
	stringToGitHubAccountIdentifier,
} from "$lib/forge/github/githubUserService.svelte";
import { describe, expect, test } from "vitest";
import type { GithubAccountIdentifier } from "@gitbutler/but-sdk";

const UNIT_SEP = "\u001F";

describe("github account identifier serialization", () => {
	test("serializes an oAuthUsername account", () => {
		const account: GithubAccountIdentifier = {
			type: "oAuthUsername",
			info: {
				username: "octocat",
			},
		};

		expect(githubAccountIdentifierToString(account)).toBe(`oAuthUsername${UNIT_SEP}octocat`);
	});

	test("serializes a patUsername account", () => {
		const account: GithubAccountIdentifier = {
			type: "patUsername",
			info: {
				username: "alice",
			},
		};

		expect(githubAccountIdentifierToString(account)).toBe(`patUsername${UNIT_SEP}alice`);
	});

	test("serializes an enterprise account", () => {
		const account: GithubAccountIdentifier = {
			type: "enterprise",
			info: {
				host: "github.example.com",
				username: "bob",
			},
		};

		expect(githubAccountIdentifierToString(account)).toBe(
			`enterprise${UNIT_SEP}github.example.com${UNIT_SEP}bob`,
		);
	});
});

describe("github account identifier deserialization", () => {
	test("deserializes an oAuthUsername account", () => {
		expect(stringToGitHubAccountIdentifier(`oAuthUsername${UNIT_SEP}octocat`)).toStrictEqual({
			type: "oAuthUsername",
			info: {
				username: "octocat",
			},
		});
	});

	test("deserializes a patUsername account", () => {
		expect(stringToGitHubAccountIdentifier(`patUsername${UNIT_SEP}alice`)).toStrictEqual({
			type: "patUsername",
			info: {
				username: "alice",
			},
		});
	});

	test("deserializes an enterprise account", () => {
		expect(
			stringToGitHubAccountIdentifier(`enterprise${UNIT_SEP}github.example.com${UNIT_SEP}bob`),
		).toStrictEqual({
			type: "enterprise",
			info: {
				host: "github.example.com",
				username: "bob",
			},
		});
	});

	test("returns null for malformed or unsupported values", () => {
		expect(stringToGitHubAccountIdentifier("patUsername")).toBeNull();
		expect(stringToGitHubAccountIdentifier(`enterprise${UNIT_SEP}only-host`)).toBeNull();
		expect(stringToGitHubAccountIdentifier(`selfHosted${UNIT_SEP}host${UNIT_SEP}user`)).toBeNull();
	});
});

describe("github account identifier round-trip", () => {
	test("round-trips supported account types", () => {
		const accounts: GithubAccountIdentifier[] = [
			{
				type: "oAuthUsername",
				info: {
					username: "octocat",
				},
			},
			{
				type: "patUsername",
				info: {
					username: "alice",
				},
			},
			{
				type: "enterprise",
				info: {
					host: "github.example.com",
					username: "bob",
				},
			},
		];

		for (const account of accounts) {
			const serialized = githubAccountIdentifierToString(account);
			expect(stringToGitHubAccountIdentifier(serialized)).toStrictEqual(account);
		}
	});
});
