import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";

export const COMMIT_MESSAGE_SYSTEM_PROMPT =
	"You write Git commit messages. Return only the commit message, without Markdown or commentary.";
const COMMIT_MESSAGE_DIFF_LIMIT = 5_000;

export const commitMessageGenerationButtonState = ({
	enabled,
	configured,
	busy,
	changeCount,
}: {
	enabled: boolean;
	configured: boolean;
	busy: boolean;
	changeCount: number;
}) => ({ visible: enabled && configured, disabled: busy || changeCount === 0 });

export const changesSelectedForCommit = (
	changes: Array<TreeChange>,
	checkedPaths: ReadonlySet<string>,
): Array<TreeChange> =>
	checkedPaths.size === 0 ? changes : changes.filter((change) => checkedPaths.has(change.path));

const formatPatch = (change: TreeChange, patch: UnifiedPatch | null): string => {
	const heading = `File: ${change.path} (${change.status.type})`;
	if (patch === null) return `${heading}\nDiff unavailable.`;
	if (patch.type === "Binary") return `${heading}\nBinary file changed.`;
	if (patch.type === "TooLarge")
		return `${heading}\nDiff omitted because the file is ${patch.subject.sizeInBytes} bytes.`;
	return `${heading}\n${patch.subject.hunks.map((hunk) => hunk.diff).join("\n")}`;
};

export const buildCommitMessagePrompt = (
	instructions: string,
	changes: Array<TreeChange>,
	patches: Array<UnifiedPatch | null>,
): string => {
	const diff = changes
		.map((change, index) => formatPatch(change, patches[index] ?? null))
		.join("\n\n")
		.slice(0, COMMIT_MESSAGE_DIFF_LIMIT);
	return `${instructions.trim()}\n\nSelected changes:\n\`\`\`diff\n${diff}\n\`\`\``;
};

/** Streams partial values and restores the previous value if a started stream fails. */
export const streamCommitMessage = async (
	stream: (onToken: (token: string) => void) => Promise<string>,
	onValue: (value: string) => void,
	previousValue: string,
): Promise<string> => {
	let partial = "";
	try {
		const response = await stream((token) => {
			partial += token;
			onValue(partial);
		});
		onValue(response);
		return response;
	} catch (error) {
		if (partial.length > 0) onValue(previousValue);
		throw error;
	}
};
