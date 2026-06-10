import { AlertDialog } from "@base-ui/react";
import { useEffect, useRef, useState } from "react";
import type { FC, SyntheticEvent } from "react";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";
import styles from "./AskpassPromptDialog.module.css";
import { AskpassPromptEvent } from "@gitbutler/but-sdk";

const secretPromptPattern = /\b(passphrase|password|token|secret|credential)\b/i;

const isSecretPrompt = (prompt: string): boolean => secretPromptPattern.test(prompt);

function getDescription(prompt: AskpassPromptEvent): string {
	switch (prompt.context.type) {
		case "Push":
			return `push: ${prompt.prompt}`;
		case "Fetch":
			return `fetch ${prompt.prompt}`;
		case "SignedCommit":
			return `signed commit ${prompt.prompt}`;
		case "Clone":
			return `clone ${prompt.prompt}`;
	}
}

export const AskpassPromptDialog: FC = () => {
	const [prompts, setPrompts] = useState<Array<AskpassPromptEvent>>([]);
	const [response, setResponse] = useState<{ promptId: string; value: string } | null>(null);
	const [submitError, setSubmitError] = useState<{ promptId: string; message: string } | null>(
		null,
	);
	const [submitting, setSubmitting] = useState(false);
	const respondingPromptId = useRef<string | null>(null);
	const currentPrompt = prompts[0];
	const currentResponse =
		currentPrompt !== undefined && response?.promptId === currentPrompt.id ? response.value : "";
	const currentSubmitError =
		currentPrompt !== undefined && submitError?.promptId === currentPrompt.id
			? submitError.message
			: null;

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
			await window.lite.submitAskpassPromptResponse({ id: prompt.id, response: value });
			setPrompts((current) => current.filter((candidate) => candidate.id !== prompt.id));
			setSubmitError(null);
		} catch (err) {
			respondingPromptId.current = null;
			setSubmitError({
				promptId: prompt.id,
				message: err instanceof Error ? err.message : String(err),
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
		<AlertDialog.Root
			open={currentPrompt !== undefined}
			onOpenChange={(open) => {
				if (!open && currentPrompt && !submitting) void respond(currentPrompt, null);
			}}
		>
			<AlertDialog.Portal>
				<AlertDialog.Backdrop />
				<AlertDialog.Popup className={classes(uiStyles.popup, uiStyles.dialogPopup, styles.popup)}>
					{currentPrompt !== undefined && (
						<form className={styles.form} onSubmit={submit}>
							<AlertDialog.Title>Git credentials required</AlertDialog.Title>
							<AlertDialog.Description className={styles.prompt}>
								{getDescription(currentPrompt)}
							</AlertDialog.Description>
							<input
								className={styles.input}
								type={isSecretPrompt(currentPrompt.prompt) ? "password" : "text"}
								value={currentResponse}
								onChange={(event) =>
									setResponse({ promptId: currentPrompt.id, value: event.target.value })
								}
								disabled={submitting}
								aria-label="Credential response"
							/>
							{currentSubmitError !== null && (
								<p className={styles.error}>Failed to send response: {currentSubmitError}</p>
							)}
							<div className={styles.actions}>
								<button
									type="button"
									className={getButtonClassName({ variant: "ghost" })}
									disabled={submitting}
									onClick={() => void respond(currentPrompt, null)}
								>
									Cancel
								</button>
								<button
									type="submit"
									className={getButtonClassName({ variant: "pop" })}
									disabled={submitting}
								>
									Continue
								</button>
							</div>
						</form>
					)}
				</AlertDialog.Popup>
			</AlertDialog.Portal>
		</AlertDialog.Root>
	);
};
