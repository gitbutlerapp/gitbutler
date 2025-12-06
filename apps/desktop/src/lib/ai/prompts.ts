import { type Prompt, MessageRole } from '$lib/ai/types';

export const SHORT_DEFAULT_COMMIT_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `Please could you write a commit message for my changes.
Only respond with the commit message. Don't give any notes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
%{brief_style}
%{emoji_style}

Here is my git diff:
\`\`\`
%{diff}
\`\`\`
`
	}
];

export const LONG_DEFAULT_COMMIT_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `Please could you write a commit message for my changes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
Only respond with the commit message.

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
		content: `Typing utilities: Check for array of type

Added an utility function to check whether a given value is an array of a specific type.`
	},
	...SHORT_DEFAULT_COMMIT_TEMPLATE
];

export const SHORT_DEFAULT_BRANCH_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `Please could you write a branch name for my changes.
A branch name represent a brief description of the changes in the diff if given or a summary of all commit messages if given.
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
\`\`\`
%{diff}
\`\`\`


And here are the commit messages:

%{commits}
`
	}
];

export const LONG_DEFAULT_BRANCH_TEMPLATE: Prompt = [
	{
		role: MessageRole.User,
		content: `Please could you write a branch name for my changes.
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

export const DEFAULT_PR_SUMMARY_MAIN_DIRECTIVE =
	'List the most important changes. Use bullet points. Skip any other information.';

export function getPrTemplateDirective(prBodyTemplate: string | undefined): string {
	if (!prBodyTemplate) {
		return '';
	}

	return `PR_TEMPLATE:
\`\`\`
${prBodyTemplate}
\`\`\`
`;
}

export const SHORT_DEFAULT_PR_TEMPLATE: Prompt = [
	{
		role: MessageRole.System,
		content: `You're a helpful coding assistant.
Create a description for a pull request.
Use the provided context, like the COMMIT_MESSAGES, PR_TEMPLATE, current TITLE and BODY.
The list of commit messages is separated by this token: <###>.
BE CONCISE.
ONLY return the description.
Use the PR_TEMPLATE to format the description, if given.`
	},
	{
		role: MessageRole.User,
		content: `%{pr_main_directive}
%{pr_template_directive}

TITLE:
\`\`\`
%{title}
\`\`\`

BODY:
\`\`\`
%{body}
\`\`\`

COMMIT_MESSAGES:
\`\`\`
%{commit_messages}
\`\`\`
`
	}
];

export const FILL_MARKER = '<<<<<FIM>>>>>';

export const AUTOCOMPLETE_SUGGESTION_PROMPT_CONTENT = `You are a developer working on a new feature. You have made some changes to the code and are documenting them.
Use the Fill-in-the-middle approach to complete the text.
You'll be given the current text with a marker to indicate where you should continue the text.
You should only replace the marker with a helpful suggestion.
ONlY respond with the text to fill in the marker.
SUGGEST MAX 10 WORDS.
This is the marker: ${FILL_MARKER}
Return only the content to fill in the middle.
DON'T change any part of the existing message.
User the following staged changes as context:`;
