import { filesFromTransfer, uploadsToMarkdown } from "#ui/uploads.ts";
import type { Upload } from "@gitbutler/but-sdk";
import { describe, expect, test } from "vitest";

const upload = (filename: string, contentType: string): Upload => ({
	uuid: "id",
	filename,
	contentType,
	url: `https://uploads.example/${filename}`,
	public: true,
	createdAt: "2026-01-01T00:00:00Z",
	isImage: contentType.startsWith("image/"),
});

describe("uploadsToMarkdown", () => {
	test("embeds images and links everything else", () => {
		expect(
			uploadsToMarkdown([upload("shot.png", "image/png"), upload("trace.txt", "text/plain")]),
		).toBe(
			"![shot.png](https://uploads.example/shot.png)\n[trace.txt](https://uploads.example/trace.txt)",
		);
	});
});

describe("filesFromTransfer", () => {
	const transfer = (files: Array<File>) => ({ files }) as unknown as DataTransfer;

	test("has nothing to attach without a transfer", () => {
		expect(filesFromTransfer(null)).toEqual([]);
	});

	test("names a pasted screenshot, which arrives without one", () => {
		const [file] = filesFromTransfer(transfer([new File([], "", { type: "image/png" })]));
		expect(file?.name).toBe("pasted-image");
		// Renaming must not change what it is, or the upload loses its type.
		expect(file?.type).toBe("image/png");
	});

	test("keeps a dropped file's own name", () => {
		const [file] = filesFromTransfer(
			transfer([new File([], "notes.md", { type: "text/markdown" })]),
		);
		expect(file?.name).toBe("notes.md");
	});
});
