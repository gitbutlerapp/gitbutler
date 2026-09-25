import { useEffect, useRef, useState } from "react";
import type { FC, SyntheticEvent } from "react";
import { Field, Toast } from "@base-ui/react";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { FieldControlStyles, FieldRootStyles } from "@gitbutler/ui-react/Field.tsx";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import type { AskpassPromptEvent } from "@gitbutler/but-sdk";

/** What git asks for, most specific first: a password prompt can mention the username too. */
const promptKinds = [
	{ pattern: /\bpassphrase\b/i, label: "Passphrase", secret: true },
	{ pattern: /\bpassword\b/i, label: "Password", secret: true },
	{ pattern: /\btoken\b/i, label: "Token", secret: true },
	{ pattern: /\busername\b/i, label: "Username", secret: false },
] as const;

const promptKind = (prompt: string) => promptKinds.find(({ pattern }) => pattern.test(prompt));

const actionVerb = (context: AskpassPromptEvent["context"]): string => {
	switch (context.type) {
		case "Push":
			return "push";
		case "Fetch":
			return "fetch";
		case "SignedCommit":
			return "sign the commit";
		case "Clone":
			return "clone";
	}
};

/** Git names what it asks about in quotes: `Username for 'https://github.com': `. */
const promptSubject = (prompt: string): string | undefined => /'([^']+)'/.exec(prompt)?.[1];

function getDescription(prompt: AskpassPromptEvent): string {
	const verb = actionVerb(prompt.context);
	const kind = promptKind(prompt.prompt);
	if (kind === undefined) return `To ${verb}, git asks: ${prompt.prompt.trim().replace(/:$/, "")}`;

	const subject = promptSubject(prompt.prompt);
	const what = kind.label.toLowerCase();
	return subject === undefined
		? `Enter your ${what} to ${verb}.`
		: `Enter your ${what} for ${subject} to ${verb}.`;
}

export const AskpassPromptDialog: FC = () => {
	const toastManager = Toast.useToastManager();
	const [prompts, setPrompts] = useState<Array<AskpassPromptEvent>>([]);
	const [response, setResponse] = useState<{ promptId: string; value: string } | null>(null);
	const [submitting, setSubmitting] = useState(false);
	const respondingPromptId = useRef<string | null>(null);
	const currentPrompt = prompts[0];
	const currentResponse =
		currentPrompt !== undefined && response?.promptId === currentPrompt.id ? response.value : "";
	const kind = currentPrompt !== undefined ? promptKind(currentPrompt.prompt) : undefined;

	useEffect(
		() =>
			window.lite.onAskpassPrompt((event) => {
				setPrompts((current) => [...current, event]);
			}),
		[],
	);

	const respond = async (prompt: AskpassPromptEvent, value: string | null) => {
		if (respondingPromptId.current === prompt.id) return;

		respondingPromptId.current = prompt.id;
		setSubmitting(true);

		try {
			await window.lite.askpassSubmitPromptResponse({ id: prompt.id, response: value });
			setPrompts((current) => current.filter((candidate) => candidate.id !== prompt.id));
		} catch (err) {
			respondingPromptId.current = null;
			toastManager.add({
				type: "error",
				title: "Failed to send your answer to git",
				description: errorMessageForToast(err),
				priority: "high",
			});
		} finally {
			setSubmitting(false);
		}
	};

	const submit = (event: SyntheticEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (currentPrompt) void respond(currentPrompt, currentResponse);
	};

	return (
		<Modal
			alert
			open={currentPrompt !== undefined}
			onOpenChange={(open) => {
				if (!open && currentPrompt && !submitting) void respond(currentPrompt, null);
			}}
		>
			{currentPrompt !== undefined && (
				<form onSubmit={submit}>
					<ModalHeader
						title="Git credentials required"
						description={getDescription(currentPrompt)}
					/>
					<ModalBody>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Control
								render={<FieldControlStyles />}
								aria-label={kind?.label ?? "Answer"}
								type={kind?.secret === true ? "password" : "text"}
								value={currentResponse}
								onValueChange={(value) => setResponse({ promptId: currentPrompt.id, value })}
								disabled={submitting}
							/>
						</Field.Root>
					</ModalBody>
					<ModalFooter>
						<Button
							variant="ghost"
							disabled={submitting}
							onClick={() => void respond(currentPrompt, null)}
						>
							Cancel
						</Button>
						<Button type="submit" variant="gray" disabled={submitting}>
							Continue
						</Button>
					</ModalFooter>
				</form>
			)}
		</Modal>
	);
};
