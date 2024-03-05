import type { User, getCloudApiClient } from "$lib/backend/cloud"

enum MessageRole {
    User = 'user',
    System = 'system'
}

export interface PromptMessage {
    content: string
    role: MessageRole
}

const commitTemplate = `
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
`

export class Summarizer {
    constructor(private cloud: ReturnType<typeof getCloudApiClient>, private user: User) {}

    async commit(diff: string, useEmojiStyle: boolean, useBreifStyle: boolean) {
        const briefStyle = "The commit message must be only one sentence and as short as possible."
        const emojiStyle = "Make use of GitMoji in the title prefix."
        const emojiStyleDisabled = "Don't use any emoji."

        let prompt = commitTemplate.replaceAll("%{diff}", diff.slice(0, 20000))
        if (useBreifStyle) {
            prompt = prompt.replaceAll("%{brief_style}", briefStyle)
        }
        if (useEmojiStyle) {
            prompt = prompt.replaceAll("%{emoji_style}", emojiStyle)
        } else {
            prompt = prompt.replaceAll("%{emoji_style}", emojiStyleDisabled)
        }
        prompt.replaceAll("%{breif_style}", "")

        const messages: PromptMessage[] = [
            { role: MessageRole.User, content: prompt }
        ]

        const response = await this.cloud.summarize.evaluatePrompt(this.user.access_token, { messages })
        let message = response.message

        if (useBreifStyle) {
            message = message.split("\n")[0]
        }

        // trim and format output
        const firstNewLine = message.indexOf('\n');
        const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
        const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';

        return description.length > 0 ? `${summary}\n\n${description}` : summary;
    }
}
