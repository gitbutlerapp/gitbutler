import { langFromExtension, langFromFilename } from "$lib/utils/shikiHighlighter";
import { describe, expect, test } from "vitest";

describe.concurrent("langFromExtension", () => {
	test("common JS/TS extensions", () => {
		expect(langFromExtension("js")).toBe("javascript");
		expect(langFromExtension("mjs")).toBe("javascript");
		expect(langFromExtension("cjs")).toBe("javascript");
		expect(langFromExtension("ts")).toBe("typescript");
		expect(langFromExtension("mts")).toBe("typescript");
		expect(langFromExtension("tsx")).toBe("tsx");
		expect(langFromExtension("jsx")).toBe("jsx");
	});

	test("JSON variants", () => {
		expect(langFromExtension("json")).toBe("json");
		expect(langFromExtension("jsonl")).toBe("json");
		expect(langFromExtension("jsonc")).toBe("jsonc");
		expect(langFromExtension("json5")).toBe("jsonc");
	});

	test("markup and style languages", () => {
		expect(langFromExtension("html")).toBe("html");
		expect(langFromExtension("css")).toBe("css");
		expect(langFromExtension("xml")).toBe("xml");
		expect(langFromExtension("md")).toBe("markdown");
	});

	test("systems languages", () => {
		expect(langFromExtension("rs")).toBe("rust");
		expect(langFromExtension("go")).toBe("go");
		expect(langFromExtension("c")).toBe("c");
		expect(langFromExtension("h")).toBe("c");
		expect(langFromExtension("cpp")).toBe("cpp");
		expect(langFromExtension("hpp")).toBe("cpp");
		expect(langFromExtension("swift")).toBe("swift");
	});

	test("scripting languages", () => {
		expect(langFromExtension("py")).toBe("python");
		expect(langFromExtension("rb")).toBe("ruby");
		expect(langFromExtension("lua")).toBe("lua");
		expect(langFromExtension("php")).toBe("php");
		expect(langFromExtension("sh")).toBe("shellscript");
		expect(langFromExtension("bash")).toBe("shellscript");
		expect(langFromExtension("zsh")).toBe("shellscript");
	});

	test("config and data formats", () => {
		expect(langFromExtension("yaml")).toBe("yaml");
		expect(langFromExtension("yml")).toBe("yaml");
		expect(langFromExtension("toml")).toBe("toml");
		expect(langFromExtension("nix")).toBe("nix");
		expect(langFromExtension("tf")).toBe("hcl");
		expect(langFromExtension("tfvars")).toBe("hcl");
	});

	test("SFC frameworks", () => {
		expect(langFromExtension("svelte")).toBe("svelte");
		expect(langFromExtension("vue")).toBe("vue");
	});

	test("returns undefined for unknown extensions", () => {
		expect(langFromExtension("xyz")).toBeUndefined();
		expect(langFromExtension("")).toBeUndefined();
		expect(langFromExtension("docx")).toBeUndefined();
	});
});

describe.concurrent("langFromFilename", () => {
	test("maps filename extensions", () => {
		expect(langFromFilename("app.ts")).toBe("typescript");
		expect(langFromFilename("src/lib/utils.rs")).toBe("rust");
		expect(langFromFilename("styles/main.css")).toBe("css");
	});

	test("handles Dockerfiles", () => {
		expect(langFromFilename("Dockerfile")).toBe("dockerfile");
		expect(langFromFilename("Dockerfile.dev")).toBe("dockerfile");
		expect(langFromFilename("path/to/Dockerfile")).toBe("dockerfile");
		expect(langFromFilename("build.dockerfile")).toBe("dockerfile");
	});

	test("case-insensitive extension matching", () => {
		expect(langFromFilename("README.MD")).toBe("markdown");
		expect(langFromFilename("test.PY")).toBe("python");
	});

	test("returns undefined for extensionless files", () => {
		expect(langFromFilename("Makefile")).toBeUndefined();
		expect(langFromFilename("LICENSE")).toBeUndefined();
	});
});
