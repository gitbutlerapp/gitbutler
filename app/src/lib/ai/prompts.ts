import {
	type Prompt,
	MessageRole,
	PromptDirective,
	PromptTemplateParam,
	type CustomPromptDirective,
	type InternalPrompt,
	InternalPromptMessageType
} from '$lib/ai/types';

export function embedExistingCommitMessage(commitMessage: string): CustomPromptDirective {
	return `Here is the existing commit MESSAGE:\n\`\`\`\n${commitMessage.trim()}\n\`\`\`` as CustomPromptDirective;
}

export function embedPromptParameter(
	promptContent: string,
	param: PromptTemplateParam,
	value: PromptDirective | CustomPromptDirective | undefined
): string {
	const paramValue = value ? value + '\n' : '';
	const result = promptContent.replaceAll(param, paramValue);
	return result;
}

export function filterInternalPromptMessages(
	messages: InternalPrompt,
	directive: PromptDirective
): InternalPrompt {
	return messages.filter(
		(message) =>
			message.type !== InternalPromptMessageType.Example || message.forDirective === directive
	);
}

export const SHORT_DEFAULT_COMMIT_TEMPLATE: InternalPrompt = [
	{
		type: InternalPromptMessageType.MainPrompt,
		role: MessageRole.User,
		content: `${PromptTemplateParam.CreateOrRewriteMessage}
Only respond with the commit message. Don't give any notes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
${PromptTemplateParam.BriefStyle + PromptTemplateParam.EmojiStyle + PromptTemplateParam.ExistingMessage}

Here is my git diff:
\`\`\`
${PromptTemplateParam.Diff}
\`\`\`
`
	}
];

export const LONG_DEFAULT_COMMIT_TEMPLATE: InternalPrompt = [
	{
		type: InternalPromptMessageType.Example,
		forDirective: PromptDirective.WriteCommitMessage,
		role: MessageRole.User,
		content: `${PromptDirective.WriteCommitMessage}.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
Only respond with the commit message.
${PromptDirective.CommitMessageDontUseEmoji}

Here is my git diff:
\`\`\`
diff --git a/src/utils/typing.ts b/src/utils/typing.ts
index 1cbfaa2..7aeebcf 100644
--- a/src/utils/typing.ts
+++ b/src/utils/typing.ts
@@ -35,3 +35,10 @@ export function isNonEmptyObject(something: unknown): something is UnknownObject
     (Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
   );
 }
+
+export function isArrayOf<T>(
+  something: unknown,
+  check: (value: unknown) => value is T
+): something is T[] {
+  return Array.isArray(something) && something.every(check);
+}
\`\`\`
`
	},
	{
		type: InternalPromptMessageType.Example,
		forDirective: PromptDirective.WriteCommitMessage,
		role: MessageRole.Assistant,
		content: `Typing utilities: Check for array of type

Added an utility function to check whether a given value is an array of a specific type.`
	},
	{
		type: InternalPromptMessageType.Example,
		forDirective: PromptDirective.ImproveCommitMessage,
		role: MessageRole.User,
		content: `${PromptDirective.ImproveCommitMessage}.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
Only respond with the commit message.
${embedExistingCommitMessage(`Sun is out`)}

Here is my git diff:
\`\`\`
diff --git a/src/utils/typing.ts b/src/utils/typing.ts
index 1cbfaa2..7aeebcf 100644
--- a/src/utils/typing.ts
+++ b/src/utils/typing.ts
@@ -35,3 +35,10 @@ export function isNonEmptyObject(something: unknown): something is UnknownObject
     (Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
   );
 }
+
+export function isArrayOf<T>(
+  something: unknown,
+  check: (value: unknown) => value is T
+): something is T[] {
+  return Array.isArray(something) && something.every(check);
+}
\`\`\`
`
	},
	{
		type: InternalPromptMessageType.Example,
		forDirective: PromptDirective.ImproveCommitMessage,
		role: MessageRole.Assistant,
		content: `Sun is out

Added a helper function to check whether a given value is an array of a specific type.`
	},
	...SHORT_DEFAULT_COMMIT_TEMPLATE
];

export const SHORT_DEFAULT_BRANCH_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `${PromptDirective.WriteBranchName}
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
\`\`\`
${PromptTemplateParam.Diff}
\`\`\`
`
	}
];

export const LONG_DEFAULT_BRANCH_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `${PromptDirective.WriteBranchName}
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
\`\`\`
diff --git a/src/utils/typing.ts b/src/utils/typing.ts
index 1cbfaa2..7aeebcf 100644
--- a/src/utils/typing.ts
+++ b/src/utils/typing.ts
@@ -35,3 +35,10 @@ export function isNonEmptyObject(something: unknown): something is UnknownObject
     (Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
   );
 }
+
+export function isArrayOf<T>(
+  something: unknown,
+  check: (value: unknown) => value is T
+): something is T[] {
+  return Array.isArray(something) && something.every(check);
+}
\`\`\`
`
	},
	{
		role: MessageRole.Assistant,
		content: `utils-typing-is-array-of-type`
	},
	...SHORT_DEFAULT_BRANCH_TEMPLATE
];
