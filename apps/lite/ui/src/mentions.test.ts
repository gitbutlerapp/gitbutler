import { completeMention, matchMentions, mentionAtCaret } from "#ui/mentions.ts";
import type { ForgeReviewUser } from "@gitbutler/but-sdk";
import { describe, expect, test } from "vitest";

/** Spells a case as one string: `|` is the caret, `[` and `]` fence a selection. */
const parse = (spec: string) => {
	if (spec.includes("|")) {
		const start = spec.indexOf("|");
		return { text: spec.replace("|", ""), start, end: start };
	}
	const start = spec.indexOf("[");
	const end = spec.indexOf("]") - 1;
	return { text: spec.replace("[", "").replace("]", ""), start, end };
};

const user = (login: string, name: string | null = null): ForgeReviewUser => ({
	id: 0,
	login,
	name,
	email: null,
	avatarUrl: null,
	isBot: false,
});

describe("mentionAtCaret", () => {
	test("finds the mention being typed", () => {
		expect(mentionAtCaret(parse("hello @bo|"))).toEqual({ at: 6, query: "bo" });
	});

	test("opens on a bare @", () => {
		expect(mentionAtCaret(parse("@|"))).toEqual({ at: 0, query: "" });
		expect(mentionAtCaret(parse("line\n@|"))).toEqual({ at: 5, query: "" });
	});

	test("allows punctuation before the @", () => {
		expect(mentionAtCaret(parse("(@bo|"))).toEqual({ at: 1, query: "bo" });
	});

	test("takes hyphens as part of a login", () => {
		expect(mentionAtCaret(parse("@my-bot|"))).toEqual({ at: 0, query: "my-bot" });
	});

	test("leaves an email address alone", () => {
		expect(mentionAtCaret(parse("mail me@bo|"))).toBeNull();
	});

	test("ignores a caret that is not at the end of the word", () => {
		expect(mentionAtCaret(parse("@bo|b"))).toBeNull();
		expect(mentionAtCaret(parse("@|bob"))).toBeNull();
	});

	test("ignores a finished mention", () => {
		expect(mentionAtCaret(parse("hi @bob |"))).toBeNull();
	});

	test("ignores text without a mention", () => {
		expect(mentionAtCaret(parse("hi |"))).toBeNull();
		expect(mentionAtCaret(parse("|"))).toBeNull();
	});

	test("ignores a selection", () => {
		expect(mentionAtCaret(parse("@[bo]"))).toBeNull();
	});
});

describe("matchMentions", () => {
	const candidates = [
		user("xseb"),
		user("bob", "Bob Sebastian"),
		user("sebastian", "Seb"),
		user("alice"),
	];

	test("puts logins starting with the query first", () => {
		expect(matchMentions(candidates, "seb").map((u) => u.login)).toEqual([
			"sebastian",
			"xseb",
			"bob",
		]);
	});

	test("ignores case", () => {
		expect(matchMentions(candidates, "SEB").map((u) => u.login)).toEqual([
			"sebastian",
			"xseb",
			"bob",
		]);
	});

	test("offers everyone to a bare @", () => {
		expect(matchMentions(candidates, "").map((u) => u.login)).toEqual([
			"xseb",
			"bob",
			"sebastian",
			"alice",
		]);
	});

	test("caps the list", () => {
		const many = Array.from({ length: 12 }, (_, i) => user(`user${i}`));
		expect(matchMentions(many, "user")).toHaveLength(8);
	});

	test("finds no one for a stranger", () => {
		expect(matchMentions(candidates, "zed")).toEqual([]);
	});
});

describe("completeMention", () => {
	test("replaces the partial with the login and a space", () => {
		expect(completeMention(3, "bob")(parse("hi @bo|"))).toEqual({
			text: "hi @bob ",
			start: 8,
			end: 8,
		});
	});

	test("reuses a space that already follows the caret", () => {
		expect(completeMention(0, "bob")(parse("@bo| there"))).toEqual({
			text: "@bob there",
			start: 5,
			end: 5,
		});
	});

	test("keeps what follows the caret", () => {
		expect(completeMention(0, "bob")(parse("@bo|."))).toEqual({
			text: "@bob .",
			start: 5,
			end: 5,
		});
	});
});
