<script module lang="ts">
	import HunkDiff from "$components/hunkDiff/HunkDiff.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import type { DependencyLock } from "$lib/utils/diffParsing";

	const hunkStr = {
		diff: '@@ -140,7 +140,7 @@\n <Modal\n \tbind:this={modal}\n \twidth="small"\n-\ttitle="Git fetch requires input"\n+\ttitle="Git --ds--tch requires input"\n…<div class="modal-body">\n \t\t<p>Git fetch requires input</p>\n \t</div>\n </Modal>\n',
		newLines: 7,
		newStart: 140,
		oldLines: 7,
		oldStart: 140,
		linesAdded: 1,
		linesRemoved: 1,
		linesChanged: 2,
	};

	const longHunkStr = {
		diff: [
			"@@ -10,45 +10,52 @@",
			' import { onMount, onDestroy } from "svelte";',
			' import { writable } from "svelte/store";',
			" ",
			"-// Old configuration object",
			"-const CONFIG = {",
			'-\thost: "localhost",',
			"-\tport: 3000,",
			"-\tdebug: false,",
			'-\tlogLevel: "warn",',
			"-\tmaxRetries: 3,",
			"-\ttimeout: 5000,",
			"-};",
			"+// Refactored configuration using environment variables",
			'+import { env } from "$env/dynamic/private";',
			"+",
			"+interface AppConfig {",
			"+\thost: string;",
			"+\tport: number;",
			"+\tdebug: boolean;",
			'+\tlogLevel: "debug" | "info" | "warn" | "error";',
			"+\tmaxRetries: number;",
			"+\ttimeout: number;",
			"+\tbaseUrl: string;",
			"+}",
			"+",
			"+const CONFIG: AppConfig = {",
			'+\thost: env.HOST ?? "localhost",',
			"+\tport: Number(env.PORT) || 3000,",
			'+\tdebug: env.DEBUG === "true",',
			'+\tlogLevel: (env.LOG_LEVEL as AppConfig["logLevel"]) ?? "warn",',
			"+\tmaxRetries: Number(env.MAX_RETRIES) || 3,",
			"+\ttimeout: Number(env.TIMEOUT) || 5000,",
			'+\tbaseUrl: env.BASE_URL ?? "http://localhost:3000",',
			"+};",
			" ",
			" /**",
			"-\t* Initialize the application with default settings.",
			"-\t* This function sets up the basic state.",
			"+\t* Initialize the application with validated configuration.",
			"+\t* This function sets up the state and validates all config values",
			"+\t* before starting the application lifecycle.",
			" \t*/",
			"-function initialize() {",
			"-\tconst state = writable({",
			"-\t\tloading: true,",
			"-\t\tdata: null,",
			"-\t\terror: null,",
			"-\t});",
			"+async function initialize(): Promise<void> {",
			"+\tvalidateConfig(CONFIG);",
			" ",
			'-\tconsole.log("App initialized");',
			"-\treturn state;",
			"+\tconst state = writable<AppState>({",
			"+\t\tloading: true,",
			"+\t\tdata: null,",
			"+\t\terror: null,",
			"+\t\tinitialized: false,",
			"+\t});",
			"+",
			"+\tawait connectToServices(CONFIG);",
			"+\tstate.update((s) => ({ ...s, initialized: true, loading: false }));",
			"+\treturn state;",
			" }",
			" ",
			"-// Handle cleanup",
			"-function cleanup() {",
			'-\tconsole.log("Cleaning up resources");',
			"+// Handle cleanup with proper resource disposal",
			"+async function cleanup(): Promise<void> {",
			"+\tawait disconnectServices();",
			'+\tlogger.info("All resources cleaned up");',
			" }",
			" ",
			" onMount(() => {",
			"-\tinitialize();",
			"+\tinitialize().catch((err) => {",
			'+\t\tlogger.error("Failed to initialize", err);',
			"+\t});",
			" });",
			" ",
			" onDestroy(() => {",
			"-\tcleanup();",
			"+\tcleanup().catch((err) => {",
			'+\t\tlogger.error("Failed to cleanup", err);',
			"+\t});",
			" });",
		].join("\n"),
		newLines: 52,
		newStart: 10,
		oldLines: 45,
		oldStart: 10,
		linesAdded: 30,
		linesRemoved: 23,
		linesChanged: 53,
	};

	const { Story } = defineMeta({
		title: "Code / HunkDiff",
		component: HunkDiff,
		args: {
			hunkStr: hunkStr.diff,
			filePath: "test.tsx",
			tabSize: 4,
			wrapText: true,
			diffLigatures: true,
			inlineUnifiedDiffs: false,
			strongContrast: false,
			colorBlindFriendly: false,
			hideCheckboxes: false,
			selectable: false,
			draggingDisabled: false,
		},
		argTypes: {
			hunkStr: { control: "text" },
			filePath: { control: "text" },
			tabSize: { control: { type: "number", min: 1, max: 8 } },
			wrapText: { control: "boolean" },
			diffLigatures: { control: "boolean" },
			inlineUnifiedDiffs: { control: "boolean" },
			strongContrast: { control: "boolean" },
			colorBlindFriendly: { control: "boolean" },
			staged: { control: "boolean" },
			hideCheckboxes: { control: "boolean" },
			selectable: { control: "boolean" },
			draggingDisabled: { control: "boolean" },
		},
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiff {...args} />
		</div>
	{/snippet}
</Story>

<Story name="Unstaged">
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiff {...args} staged={false} />
		</div>
	{/snippet}
</Story>

<Story name="Staged (Selected)">
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiff {...args} staged={true} />
		</div>
	{/snippet}
</Story>

<Story name="With Locked Rows">
	{#snippet template(args)}
		{#snippet lockWarning(locks: DependencyLock[])}
			This line is locked by {locks.length} commit(s)
		{/snippet}

		<div class="wrap">
			<HunkDiff
				{...args}
				lineLocks={[
					{
						oldLine: 141,
						newLine: 141,
						locks: [
							{
								target: { type: "stack", subject: "refactor/modal-component" },
								commitId: "xyz789abc123",
							},
						],
					},
					{
						oldLine: 142,
						newLine: 142,
						locks: [
							{
								target: { type: "stack", subject: "feature/authentication" },
								commitId: "abc123def456",
							},
						],
					},
					{
						oldLine: 143,
						newLine: undefined,
						locks: [
							{
								target: { type: "stack", subject: "feature/data-layer" },
								commitId: "ghi789jkl012",
							},
							{
								target: { type: "unidentified" },
								commitId: "mno345pqr678",
							},
						],
					},
					{
						oldLine: 144,
						newLine: 144,
						locks: [
							{
								target: { type: "stack", subject: "fix/modal-styling" },
								commitId: "stu901vwx234",
							},
						],
					},
				]}
				{lockWarning}
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Staged Locked Rows">
	{#snippet template(args)}
		{#snippet lockWarning(locks: DependencyLock[])}
			This line is locked by {locks.length} commit(s)
		{/snippet}

		<div class="wrap">
			<HunkDiff
				{...args}
				staged={false}
				lineLocks={[
					{
						oldLine: 141,
						newLine: 141,
						locks: [
							{
								target: { type: "stack", subject: "refactor/modal-component" },
								commitId: "xyz789abc123",
							},
						],
					},
					{
						oldLine: 142,
						newLine: 142,
						locks: [
							{
								target: { type: "stack", subject: "feature/authentication" },
								commitId: "abc123def456",
							},
						],
					},
					{
						oldLine: 143,
						newLine: undefined,
						locks: [
							{
								target: { type: "stack", subject: "feature/data-layer" },
								commitId: "ghi789jkl012",
							},
							{
								target: { type: "unidentified" },
								commitId: "mno345pqr678",
							},
						],
					},
					{
						oldLine: 144,
						newLine: 144,
						locks: [
							{
								target: { type: "stack", subject: "fix/modal-styling" },
								commitId: "stu901vwx234",
							},
						],
					},
				]}
				stagedLines={[
					{
						oldLine: 142,
						newLine: 142,
					},
					{
						oldLine: 143,
						newLine: undefined,
					},
				]}
				{lockWarning}
			/>
		</div>
	{/snippet}
</Story>

<Story
	name="Long Diff"
	args={{
		hunkStr: longHunkStr.diff,
		filePath: "src/lib/app.svelte",
	}}
>
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiff {...args} />
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
	}
</style>
