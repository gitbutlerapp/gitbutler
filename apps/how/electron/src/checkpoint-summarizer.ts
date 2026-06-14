import { checkpointMessage } from "./checkpoints.js";
import { checkpointDiffForSummary } from "./git.js";
import type { Logger } from "./logger.js";
import type { CodingAgent } from "./settings.js";

const diffPayloadLimitBytes = 40 * 1024;
const fallbackSummaryTimeoutMs = 10_000;
const checkpointPrefixPattern = /^checkpoint:\s*/i;
const maxTitleLength = 72;
const maxBodyLength = 1_000;

export type CheckpointSummary = {
	title: string;
	body: string | null;
};

export type CheckpointSummaryRequest = {
	agent: Exclude<CodingAgent, "none">;
	projectId: string;
	worktreePath: string;
	diff: string;
	truncated: boolean;
	timeoutMs: number;
	signal: AbortSignal;
};

type CheckpointSummarizer = {
	summarize(request: CheckpointSummaryRequest): Promise<string>;
};

type SummarizerLogger = Pick<Logger, "info" | "error">;

export function checkpointSummaryTimeoutMs(): number {
	const override = process.env.HOW_CHECKPOINT_SUMMARY_TIMEOUT_MS;
	if (!override) return fallbackSummaryTimeoutMs;
	const parsed = Number(override);
	return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallbackSummaryTimeoutMs;
}

export function parseCheckpointSummary(output: string): CheckpointSummary | null {
	const lines = output
		.replace(/\r\n/g, "\n")
		.split("\n")
		.map((line) => line.trim());
	const firstNonEmptyLineIndex = lines.findIndex((line) => line.length > 0);
	if (firstNonEmptyLineIndex < 0) return null;

	const titleLine = lines[firstNonEmptyLineIndex];
	if (titleLine === undefined) return null;
	const rawTitle = titleLine.replace(checkpointPrefixPattern, "").trim();
	if (rawTitle.length === 0) return null;
	const title = rawTitle.slice(0, maxTitleLength).trim();
	if (title.length === 0) return null;

	const body = lines
		.slice(firstNonEmptyLineIndex + 1)
		.join("\n")
		.trim()
		.slice(0, maxBodyLength)
		.trim();
	return {
		title,
		body: body.length > 0 ? body : null,
	};
}

export function checkpointMessageFromSummary(
	date: Date,
	summary: CheckpointSummary | null,
): string {
	if (!summary) return checkpointMessage(date);
	if (!summary.body) return `Checkpoint: ${summary.title}`;
	return `Checkpoint: ${summary.title}\n\n${summary.body}`;
}

export function capDiffForSummary(diff: string): { diff: string; truncated: boolean } {
	const bytes = Buffer.byteLength(diff, "utf8");
	if (bytes <= diffPayloadLimitBytes) return { diff, truncated: false };

	const buffer = Buffer.from(diff, "utf8");
	return {
		diff: buffer.subarray(0, diffPayloadLimitBytes).toString("utf8"),
		truncated: true,
	};
}

export async function stagedDiffForSummary(projectId: string): Promise<{
	diff: string;
	truncated: boolean;
	originalByteCount: number;
}> {
	const { diff, originalByteCount } = await checkpointDiffForSummary(projectId);
	const capped = capDiffForSummary(diff);
	return {
		...capped,
		originalByteCount,
	};
}

export async function checkpointMessageForStagedChanges({
	agent,
	date,
	logger,
	projectId,
	worktreePath,
}: {
	agent: CodingAgent;
	date: Date;
	logger: SummarizerLogger;
	projectId: string;
	worktreePath: string;
}): Promise<string> {
	if (agent === "none") return checkpointMessage(date);

	const timeoutMs = checkpointSummaryTimeoutMs();
	const diff = await stagedDiffForSummary(projectId);
	logger.info("Preparing AI checkpoint summary", {
		agent,
		diffByteCount: diff.originalByteCount,
		diffTruncated: diff.truncated,
		timeoutMs,
	});

	const summarizer = createCheckpointSummarizer();
	try {
		const output = await withAbortableTimeout(
			async (signal) =>
				await summarizer.summarize({
					agent,
					projectId,
					worktreePath,
					diff: diff.diff,
					truncated: diff.truncated,
					timeoutMs,
					signal,
				}),
			timeoutMs,
		);
		const summary = parseCheckpointSummary(output);
		if (!summary) {
			logger.info("AI checkpoint summary was not usable; falling back to timestamp", { agent });
			return checkpointMessage(date);
		}
		logger.info("AI checkpoint summary succeeded", { agent });
		return checkpointMessageFromSummary(date, summary);
	} catch (error) {
		logger.error("AI checkpoint summary failed; falling back to timestamp", error, { agent });
		return checkpointMessage(date);
	}
}

function createCheckpointSummarizer(): CheckpointSummarizer {
	if (
		process.env.HOW_E2E_CHECKPOINT_SUMMARY ||
		process.env.HOW_E2E_CHECKPOINT_SUMMARY_DELAY_MS ||
		process.env.HOW_E2E_CHECKPOINT_SUMMARY_ERROR
	)
		return new FakeCheckpointSummarizer();
	return new AgentSdkCheckpointSummarizer();
}

class FakeCheckpointSummarizer implements CheckpointSummarizer {
	async summarize(): Promise<string> {
		const delay = Number(process.env.HOW_E2E_CHECKPOINT_SUMMARY_DELAY_MS ?? 0);
		if (Number.isFinite(delay) && delay > 0)
			await new Promise((resolve) => setTimeout(resolve, delay));
		if (process.env.HOW_E2E_CHECKPOINT_SUMMARY_ERROR)
			throw new Error("Fake checkpoint summary failed.");
		return process.env.HOW_E2E_CHECKPOINT_SUMMARY ?? "";
	}
}

class AgentSdkCheckpointSummarizer implements CheckpointSummarizer {
	async summarize(request: CheckpointSummaryRequest): Promise<string> {
		const prompt = checkpointSummaryPrompt(request);
		if (request.agent === "codex") return await summarizeWithCodex(request, prompt);
		return await summarizeWithClaude(request, prompt);
	}
}

async function summarizeWithCodex(
	request: CheckpointSummaryRequest,
	prompt: string,
): Promise<string> {
	const { Codex } = await import("@openai/codex-sdk");
	const codex = new Codex();
	const thread = codex.startThread({
		workingDirectory: request.worktreePath,
		skipGitRepoCheck: true,
		sandboxMode: "read-only",
		approvalPolicy: "never",
		networkAccessEnabled: false,
		webSearchMode: "disabled",
	});
	const turn = await thread.run(prompt, { signal: request.signal });
	return turn.finalResponse;
}

async function summarizeWithClaude(
	request: CheckpointSummaryRequest,
	prompt: string,
): Promise<string> {
	const { query } = await import("@anthropic-ai/claude-agent-sdk");
	const abortController = new AbortController();
	forwardAbort(request.signal, abortController);

	for await (const message of query({
		prompt,
		options: {
			abortController,
			cwd: request.worktreePath,
			maxTurns: 1,
			permissionMode: "dontAsk",
			persistSession: false,
			tools: [],
		},
	})) {
		if (message.type !== "result") continue;
		if (message.subtype === "success") return message.result;
		throw new Error(message.errors.join("\n") || "Claude checkpoint summary failed.");
	}

	throw new Error("Claude checkpoint summary ended without a result.");
}

export function checkpointSummaryPrompt(request: CheckpointSummaryRequest): string {
	const truncationNotice = request.truncated
		? "The diff was truncated. Summarize only what is visible."
		: "Summarize the complete diff below.";
	return [
		"You are naming an automatic Checkpoint for a software project.",
		"Return strict plain text only.",
		"The first line must be a short title without the word Checkpoint.",
		"Any remaining lines should be a concise summary body.",
		truncationNotice,
		"",
		request.diff,
	].join("\n");
}

async function withAbortableTimeout<T>(
	start: (signal: AbortSignal) => Promise<T>,
	timeoutMs: number,
): Promise<T> {
	const controller = new AbortController();
	if (timeoutMs === 0) return await start(controller.signal);
	let timeout: NodeJS.Timeout | null = null;
	try {
		return await Promise.race([
			start(controller.signal),
			new Promise<T>((_, reject) => {
				timeout = setTimeout(() => {
					controller.abort();
					reject(new Error("Checkpoint summary timed out."));
				}, timeoutMs);
			}),
		]);
	} finally {
		if (timeout) clearTimeout(timeout);
	}
}

function forwardAbort(source: AbortSignal, target: AbortController): void {
	if (source.aborted) {
		target.abort();
		return;
	}
	source.addEventListener("abort", () => target.abort(), { once: true });
}
