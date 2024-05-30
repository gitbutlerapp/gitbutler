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
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
\`\`\`
%{diff}
\`\`\`
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
