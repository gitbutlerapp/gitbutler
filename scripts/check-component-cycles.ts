/**
 * Detects circular dependencies between component folders.
 *
 * Builds a folder-level dependency graph from $components/ imports across all
 * .svelte files, then runs DFS cycle detection. Any cycle means two folders
 * depend on each other, which is a sign of misplaced components.
 *
 * Usage (tsx resolves from apps/desktop/node_modules):
 *   pnpm -F @gitbutler/desktop check-cycles
 *   pnpm -F @gitbutler/desktop check-cycles -- --verbose
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const COMPONENTS_DIR = path.resolve(__dirname, "../apps/desktop/src/components");
const VERBOSE = process.argv.includes("--verbose");

// ── 1. Collect all .svelte files ────────────────────────────────────────────

function findSvelteFiles(dir: string): string[] {
	const results: string[] = [];
	for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
		const full = path.join(dir, entry.name);
		if (entry.isDirectory()) results.push(...findSvelteFiles(full));
		else if (entry.name.endsWith(".svelte")) results.push(full);
	}
	return results;
}

// ── 2. Build folder → folder dependency graph ───────────────────────────────

function folderOf(filePath: string): string {
	const rel = path.relative(COMPONENTS_DIR, filePath);
	const parts = rel.split(path.sep);
	// Files directly in COMPONENTS_DIR have no subfolder — treat as "(root)"
	return parts.length > 1 ? parts[0] : "(root)";
}

const importRe = /from\s+['"](\$components\/[^'"]+)['"]/g;

function buildGraph(files: string[]): Map<string, Set<string>> {
	const graph = new Map<string, Set<string>>();

	for (const file of files) {
		const folder = folderOf(file);
		if (!graph.has(folder)) graph.set(folder, new Set());

		const content = fs.readFileSync(file, "utf-8");
		let match: RegExpExecArray | null;
		importRe.lastIndex = 0;
		while ((match = importRe.exec(content)) !== null) {
			// $components/folder/Component.svelte  →  folder
			// $components/Component.svelte         →  (root)
			const importPath = match[1].replace("$components/", "");
			const parts = importPath.split("/");
			const importedFolder = parts.length > 1 ? parts[0] : "(root)";

			if (importedFolder !== folder) {
				graph.get(folder)!.add(importedFolder);
				if (VERBOSE) {
					console.log(
						`  ${folder}/${path.basename(file)} → ${importedFolder}/${parts.slice(1).join("/")}`,
					);
				}
			}
		}
	}

	return graph;
}

// ── 3. Cycle detection (DFS with three-color marking) ───────────────────────

type Color = "white" | "gray" | "black";

function findCycles(graph: Map<string, Set<string>>): string[][] {
	const color = new Map<string, Color>();
	const parent = new Map<string, string | null>();
	const cycles: string[][] = [];

	for (const node of graph.keys()) color.set(node, "white");

	function dfs(node: string) {
		color.set(node, "gray");
		for (const neighbour of graph.get(node) ?? []) {
			if (!color.has(neighbour)) {
				// Node from outside our graph (e.g. shared/ imported but never imports)
				color.set(neighbour, "white");
				graph.set(neighbour, new Set());
			}
			if (color.get(neighbour) === "gray") {
				// Back edge → cycle found; reconstruct it
				const cycle: string[] = [neighbour];
				let cur: string | null | undefined = node;
				while (cur && cur !== neighbour) {
					cycle.unshift(cur);
					cur = parent.get(cur);
				}
				cycle.unshift(neighbour);
				cycles.push(cycle);
			} else if (color.get(neighbour) === "white") {
				parent.set(neighbour, node);
				dfs(neighbour);
			}
		}
		color.set(node, "black");
	}

	for (const node of graph.keys()) {
		if (color.get(node) === "white") dfs(node);
	}

	return cycles;
}

// ── 4. Main ──────────────────────────────────────────────────────────────────

const files = findSvelteFiles(COMPONENTS_DIR);
console.log(`Scanning ${files.length} components in ${COMPONENTS_DIR}\n`);

const graph = buildGraph(files);

if (VERBOSE) {
	console.log("\n── Folder dependency graph ──");
	for (const [folder, deps] of [...graph.entries()].sort()) {
		if (deps.size > 0) {
			console.log(`  ${folder} → ${[...deps].sort().join(", ")}`);
		}
	}
	console.log();
}

const cycles = findCycles(graph);

if (cycles.length === 0) {
	console.log("✓ No circular dependencies between folders.");
} else {
	console.log(`✗ Found ${cycles.length} circular dependency chain(s):\n`);
	for (const cycle of cycles) {
		console.log("  " + cycle.join(" → "));
	}
	console.log(
		"\nEach cycle means components in those folders import each other — a sign of misplaced components.",
	);
	process.exit(1);
}
