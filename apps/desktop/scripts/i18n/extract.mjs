/**
 * Extracts user-visible English strings from Svelte components and rewrites
 * them as `t("key")` calls against $lib/i18n.
 *
 * - Template text nodes become {t("key")} expressions.
 * - Translatable attribute values (placeholder, aria-label, …) become {t("key")}.
 * - Keys are derived from the English text; en.json is regenerated in sorted order.
 *
 * Usage: node scripts/i18n/extract.mjs [--dry] [--verbose]
 */
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { parse } from "svelte/compiler";

const require_ = createRequire(import.meta.url);
// estree-walker is only present as a transitive pnpm dependency, so resolve
// it through svelte (a direct dependency of apps/desktop). Both the 2.x and
// 3.x `walk` APIs used here (enter/leave, this.skip) are identical.
const requireSvelte = createRequire(require_.resolve("svelte/package.json"));
const { walk } = require_(requireSvelte.resolve("estree-walker"));

const ROOT = path.resolve(import.meta.dirname, "../../src");
const LOCALES_DIR = path.resolve(import.meta.dirname, "../../src/lib/i18n/locales");
const OUT_EN = path.join(LOCALES_DIR, "en.json");
const OUT_ZH = path.join(LOCALES_DIR, "zh-CN.json");
const DRY = process.argv.includes("--dry");
const VERBOSE = process.argv.includes("--verbose");

const SKIP_TAGS = new Set([
	"script",
	"style",
	"template",
	"svg",
	"path",
	"circle",
	"rect",
	"line",
	"polyline",
	"polygon",
	"g",
	"defs",
	"clipPath",
	"mask",
	"use",
	"ellipse",
]);

// Attributes whose values are user-visible copy
const TRANSLATABLE_ATTRS = new Set([
	"placeholder",
	"aria-label",
	"aria-description",
	"alt",
	"heading",
	"label",
	"caption",
	"tooltip",
]);

// English text that should not become a translation entry
function isCopyText(text) {
	const t = text.trim();
	if (t.length < 2) return false;
	if (t.length > 300) return false;
	// Needs at least one uppercase letter or space (filters css values, ids)
	if (!/[A-Z]/.test(t) && !/\s/.test(t)) return false;
	if (!/[A-Za-z]/.test(t)) return false;
	if (/^https?:\/\//.test(t)) return false;
	if (/^[A-Z]:\//.test(t)) return false;
	// css-class-like tokens (dash-joined words, no natural phrasing)
	if (
		/^[-\w]+( [-\w]+)*$/.test(t) &&
		!/[A-Z]/.test(t) &&
		t.split(" ").every((w) => /^[\w-]+$/.test(w))
	) {
		// allow natural copy that happens to be lowercase by requiring a space-joined phrase ≥2 words
		const words = t.split(/\s+/);
		if (words.length < 2) return false;
		// multi-word all-lowercase without punctuation is usually css/identifiers
		if (!/[.,!?'’"()]/.test(t)) return false;
	}
	// pure identifiers / emails / single tokens with no space stay untranslated
	if (!/\s/.test(t) && /^[A-Za-z0-9_@:.\-+/]+$/.test(t)) return false;
	if (/^[\d\s.,:/%-]+$/.test(t)) return false;
	// git branch-ish / flag-ish
	if (/^(true|false|null|undefined)$/i.test(t)) return false;
	return true;
}

function slugify(text) {
	const s = text
		.replace(/[^\w\s-]/g, " ")
		.trim()
		.toLowerCase()
		.replace(/[\s_-]+/g, "-")
		.replace(/^-+|-+$/g, "");
	return s.slice(0, 60) || "text";
}

/** text -> key */
const keyCache = new Map();
const usedKeys = new Set();
const messages = new Map();

function keyFor(text) {
	if (keyCache.has(text)) return keyCache.get(text);
	let base = slugify(text);
	if (!base || !/^[a-z]/.test(base)) base = `t-${base}`;
	let key = base;
	let n = 2;
	while (usedKeys.has(key)) key = `${base}-${n++}`;
	usedKeys.add(key);
	keyCache.set(text, key);
	return key;
}

function register(text) {
	const key = keyFor(text);
	if (!messages.has(key)) messages.set(key, text);
	return key;
}

function listFiles(dir) {
	let out = [];
	for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
		const p = path.join(dir, entry.name);
		if (entry.isDirectory()) out.push(...listFiles(p));
		else if (entry.name.endsWith(".svelte")) out.push(p);
	}
	return out;
}

function applyEdits(source, edits) {
	edits.sort((a, b) => b.start - a.start);
	for (const e of edits) {
		source = source.slice(0, e.start) + e.text + source.slice(e.end);
	}
	return source;
}

const files = listFiles(ROOT);
let totalEdits = 0;
const perFile = [];

for (const file of files) {
	let source = fs.readFileSync(file, "utf8");
	let ast;
	try {
		ast = parse(source, { filename: file, modern: true });
	} catch (e) {
		console.error(`SKIP (parse error) ${file}: ${e.message}`);
		continue;
	}
	const edits = [];

	walk(ast.fragment, {
		enter(node, parent) {
			// skip <script>/<style>/svg innards entirely
			if ((node.type === "RegularElement" || node.type === "Element") && SKIP_TAGS.has(node.name)) {
				this.skip();
				return;
			}
			if (node.type === "Text") {
				// Text nodes inside attribute values are handled with the attribute
				if (parent?.type === "Attribute") return;
				const raw = node.raw ?? node.data ?? "";
				const trimmed = raw.trim();
				if (!trimmed || !isCopyText(trimmed)) return;
				// Multi-line / whitespace-padded text: replace only the trimmed span
				const startOff = node.start + raw.indexOf(trimmed[0]);
				const endOff = node.start + raw.lastIndexOf(trimmed.slice(-1)) + 1;
				const key = register(trimmed);
				edits.push({ start: startOff, end: endOff, text: `{t("${key}")}` });
				totalEdits++;
				return;
			}
			if (node.type === "Attribute" && TRANSLATABLE_ATTRS.has(node.name.toLowerCase())) {
				const vals = Array.isArray(node.value) ? node.value : node.value ? [node.value] : [];
				if (vals.length >= 1 && vals.every((v) => v.type === "Text")) {
					// `data` excludes the surrounding quotes; `raw` includes them
					const full = vals.map((v) => v.raw ?? v.data ?? "").join("");
					const trimmed = full.trim();
					if (isCopyText(trimmed)) {
						const key = register(trimmed);
						// Replace the whole quoted region so no stray quote tails remain
						edits.push({
							start: vals[0].start,
							end: vals[vals.length - 1].end,
							text: `{t("${key}")}`,
						});
						totalEdits++;
					}
				}
			}
		},
	});

	if (edits.length) {
		perFile.push([path.relative(process.cwd(), file), edits.length]);
		if (!DRY) {
			const next = applyEdits(source, edits);
			let final = next;
			if (!/from "\$lib\/i18n"/.test(final)) {
				const scriptOpen = final.match(/<script[^>]*>/);
				if (scriptOpen) {
					const idx = final.indexOf(scriptOpen[0]) + scriptOpen[0].length;
					final = final.slice(0, idx) + `\n\timport { t } from "$lib/i18n";\n` + final.slice(idx);
				} else {
					final = `<script lang="ts">\n\timport { t } from "$lib/i18n";\n</script>\n` + final;
				}
			}
			fs.writeFileSync(file, final);
		}
	}
}

/** Read a locale dictionary, returning {} when the file does not exist yet. */
function readJson(file) {
	try {
		return JSON.parse(fs.readFileSync(file, "utf8"));
	} catch (e) {
		if (e.code !== "ENOENT") throw e;
		return {};
	}
}

// Merge this run's extracted strings into the existing dictionaries so keys
// extracted in earlier runs survive; new keys fall back to English in zh-CN
// until a translator fills them in.
const enExisting = readJson(OUT_EN);
const zhExisting = readJson(OUT_ZH);
const en = { ...enExisting };
const zh = { ...zhExisting };
for (const [key, text] of messages) {
	if (!(key in en)) en[key] = text;
	if (!(key in zh)) zh[key] = text;
}
if (!DRY) {
	const sortEntries = (obj) =>
		Object.fromEntries(Object.entries(obj).sort((a, b) => a[0].localeCompare(b[0])));
	fs.mkdirSync(LOCALES_DIR, { recursive: true });
	fs.writeFileSync(OUT_EN, JSON.stringify(sortEntries(en), null, "\t") + "\n");
	fs.writeFileSync(OUT_ZH, JSON.stringify(sortEntries(zh), null, "\t") + "\n");
}
console.log(
	`files with edits: ${perFile.length}, total edits: ${totalEdits}, new keys: ${
		messages.size - [...messages.keys()].filter((k) => k in enExisting).length
	}, en total: ${Object.keys(en).length}`,
);
if (VERBOSE) {
	for (const [f, n] of perFile) console.log(`  ${f}: ${n}`);
}
