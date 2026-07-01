import {
	gitlabAccountIdentifierToString,
	stringToGitLabAccountIdentifier,
} from "$lib/forge/gitlab/gitlabUserService.svelte";
import { describe, expect, test } from "vitest";
import type { GitlabAccountIdentifier } from "@gitbutler/but-sdk";

const UNIT_SEP = "\u001F";

describe("gitlab account identifier serialization", () => {
	test("serializes a patUsername account", () => {
		const account: GitlabAccountIdentifier = {
			type: "patUsername",
			info: {
				username: "octocat",
			},
		};

		expect(gitlabAccountIdentifierToString(account)).toBe(`patUsername${UNIT_SEP}octocat`);
	});

	test("serializes a selfHosted account", () => {
		const account: GitlabAccountIdentifier = {
			type: "selfHosted",
			info: {
				host: "gitlab.example.com",
				username: "alice",
			},
		};

		expect(gitlabAccountIdentifierToString(account)).toBe(
			`selfHosted${UNIT_SEP}gitlab.example.com${UNIT_SEP}alice`,
		);
	});
});

describe("gitlab account identifier deserialization", () => {
	test("deserializes a patUsername account", () => {
		expect(stringToGitLabAccountIdentifier(`patUsername${UNIT_SEP}octocat`)).toStrictEqual({
			type: "patUsername",
			info: {
				username: "octocat",
			},
		});
	});

	test("deserializes a selfHosted account", () => {
		expect(
			stringToGitLabAccountIdentifier(`selfHosted${UNIT_SEP}gitlab.example.com${UNIT_SEP}alice`),
		).toStrictEqual({
			type: "selfHosted",
			info: {
				host: "gitlab.example.com",
				username: "alice",
			},
		});
	});

	test("returns null for malformed or unsupported values", () => {
		expect(stringToGitLabAccountIdentifier("patUsername")).toBeNull();
		expect(stringToGitLabAccountIdentifier("enterprise\u001Fhost\u001Fuser")).toBeNull();
		expect(stringToGitLabAccountIdentifier(`selfHosted${UNIT_SEP}only-host`)).toBeNull();
	});
});

describe("gitlab account identifier round-trip", () => {
	test("round-trips supported account types", () => {
		const accounts: GitlabAccountIdentifier[] = [
			{
				type: "patUsername",
				info: {
					username: "octocat",
				},
			},
			{
				type: "selfHosted",
				info: {
					host: "gitlab.example.com",
					username: "alice",
				},
			},
		];

		for (const account of accounts) {
			const serialized = gitlabAccountIdentifierToString(account);
			expect(stringToGitLabAccountIdentifier(serialized)).toStrictEqual(account);
		}
	});
});
