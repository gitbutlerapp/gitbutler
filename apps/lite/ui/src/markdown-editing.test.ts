import {
	bold,
	bulletList,
	heading2,
	heading3,
	insert,
	link,
	numberList,
	plainText,
	quote,
	taskList,
	type MarkdownCommand,
} from "#ui/markdown-editing.ts";
import { describe, expect, test } from "vitest";

/**
 * Spells a case as one string: `|` is a collapsed caret, `[` and `]` fence a
 * selection. Keeps the expectation readable as the text the user would see.
 */
const parse = (spec: string) => {
	if (spec.includes("|")) {
		const start = spec.indexOf("|");
		return { text: spec.replace("|", ""), start, end: start };
	}
	const start = spec.indexOf("[");
	const end = spec.indexOf("]") - 1;
	return { text: spec.replace("[", "").replace("]", ""), start, end };
};

const format = ({ text, start, end }: { text: string; start: number; end: number }) =>
	start === end
		? `${text.slice(0, start)}|${text.slice(start)}`
		: `${text.slice(0, start)}[${text.slice(start, end)}]${text.slice(end)}`;

const apply = (command: MarkdownCommand, spec: string) => format(command(parse(spec)));

describe("inline commands", () => {
	test("wraps the selection", () => {
		expect(apply(bold, "make [this] bold")).toBe("make **[this]** bold");
	});

	test("unwraps markers inside the selection", () => {
		expect(apply(bold, "make [**this**] bold")).toBe("make [this] bold");
	});

	test("unwraps markers just outside the selection", () => {
		expect(apply(bold, "make **[this]** bold")).toBe("make [this] bold");
	});

	test("wraps an empty selection so the caret lands between the markers", () => {
		expect(apply(bold, "type |here")).toBe("type **|**here");
	});

	test("link leaves the placeholder URL selected", () => {
		expect(apply(link, "see [the docs] now")).toBe("see [the docs]([url]) now");
	});
});

describe("block commands", () => {
	test("prefixes every line the selection touches", () => {
		expect(apply(bulletList, "on[e\ntw]o")).toBe("[- one\n- two]");
	});

	test("numbers ordered list items from one", () => {
		expect(apply(numberList, "on[e\ntw]o")).toBe("[1. one\n2. two]");
	});

	test("toggles an applied prefix back off", () => {
		expect(apply(bulletList, "- on[e\n- tw]o")).toBe("[one\ntwo]");
	});

	test("replaces a competing list marker rather than stacking it", () => {
		expect(apply(taskList, "- on|e")).toBe("[- [ ] one]");
	});

	test("converts a task line to a bullet instead of toggling it off", () => {
		expect(apply(bulletList, "- [ ] on|e")).toBe("[- one]");
	});

	test("renumbers a task line rather than toggling it off", () => {
		expect(apply(numberList, "- [ ] on|e")).toBe("[1. one]");
	});

	test("applies to the line holding a collapsed caret", () => {
		expect(apply(quote, "before\nmid|dle\nafter")).toBe("before\n[> middle]\nafter");
	});

	test("swaps one heading level for another", () => {
		expect(apply(heading3, "## tit|le")).toBe("[### title]");
	});

	test("plain text strips the heading marker without adding one", () => {
		expect(apply(plainText, "### tit|le")).toBe("[title]");
	});

	test("re-applying the same heading removes it", () => {
		expect(apply(heading2, "## tit|le")).toBe("[title]");
	});
});

describe("insert", () => {
	test("drops the snippet at the caret and lands after it", () => {
		expect(apply(insert("![shot](url)"), "see: |")).toBe("see: ![shot](url)|");
	});

	test("replaces the selection", () => {
		expect(apply(insert("![shot](url)"), "see: [placeholder]!")).toBe("see: ![shot](url)|!");
	});
});
