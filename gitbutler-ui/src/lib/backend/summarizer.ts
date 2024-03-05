import type { AIProvider } from '$lib/backend/ai_providers';

const diffLengthLimit = 20000;

const defaultCommitTemplate = `
Please could you write a commit message for my changes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Always use semantic commit prefixes.
Hard wrap lines at 72 characters.
%{brief_style}
%{emoji_style}

Here is my diff:
%{diff}
`;

const defaultBranchTemplate = `
Please could you write a branch name for my changes.
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.

Here is my diff:
%{diff}
`;

export class Summarizer {
	constructor(private aiProvider: AIProvider) {}

	async commit(
		diff: string,
		useEmojiStyle: boolean,
		useBriefStyle: boolean,
		commitTemplate?: string
	) {
		let prompt = (commitTemplate || defaultCommitTemplate).replaceAll(
			'%{diff}',
			diff.slice(0, diffLengthLimit)
		);

		if (useBriefStyle) {
			prompt = prompt.replaceAll(
				'%{brief_style}',
				'The commit message must be only one sentence and as short as possible.'
			);
		} else {
			prompt = prompt.replaceAll('%{brief_style}', '');
		}
		if (useEmojiStyle) {
			prompt = prompt.replaceAll('%{emoji_style}', 'Make use of GitMoji in the title prefix.');
		} else {
			prompt = prompt.replaceAll('%{emoji_style}', "Don't use any emoji.");
		}

		let message = await this.aiProvider.evaluate(prompt);

		if (useBriefStyle) {
			message = message.split('\n')[0];
		}

		const firstNewLine = message.indexOf('\n');
		const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
		const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';

		return description.length > 0 ? `${summary}\n\n${description}` : summary;
	}

	async branch(diff: string, branchTemplate?: string) {
		const prompt = (branchTemplate || defaultBranchTemplate).replaceAll(
			'%{diff}',
			diff.slice(0, diffLengthLimit)
		);

		let message = await this.aiProvider.evaluate(prompt);

		message = message.replaceAll(' ', '-');
		message = message.replaceAll('\n', '-');
		return message;
	}
}
