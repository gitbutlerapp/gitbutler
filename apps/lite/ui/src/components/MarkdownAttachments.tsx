import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";
import { useUploadFiles } from "#ui/api/mutations.ts";
import { userProfileQueryOptions } from "#ui/api/queries.ts";
import * as md from "#ui/markdown-editing.ts";
import { applyToTextarea } from "#ui/markdown-textarea.ts";
import { ACCEPTED_FILE_TYPES, filesFromTransfer, uploadsToMarkdown } from "#ui/uploads.ts";
import { AlertDialog, Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type FC, type RefObject, useEffect, useRef, useState } from "react";
import styles from "./MarkdownAttachments.module.css";

type Props = {
	/** The textarea the uploaded files' markdown is inserted into. */
	targetRef: RefObject<HTMLTextAreaElement | null>;
	/** Receives the rewritten source, for the owner's controlled state. */
	onInput: (value: string) => void;
	disabled?: boolean;
};

/**
 * Attach files to a markdown textarea: by the paperclip, by pasting, or by
 * dropping onto the field.
 *
 * Uploads go to gitbutler.com and are public — that is what makes them
 * loadable from the forge's rendered body — so every batch is confirmed
 * before it leaves the machine. Signing in is what supplies the account the
 * upload is billed to, so signed out the action stays visible but disabled
 * rather than failing at the end of a drop.
 */
export const MarkdownAttachments: FC<Props> = (p) => {
	const { data: profile } = useQuery(userProfileQueryOptions);
	const uploadFiles = useUploadFiles();
	const inputRef = useRef<HTMLInputElement | null>(null);
	const [pending, setPending] = useState<Array<File>>([]);

	const signedIn = profile !== undefined && profile !== null;
	const enabled = signedIn && p.disabled !== true && !uploadFiles.isPending;

	// Paste and drop reach the textarea itself, so they are listened for on the
	// node rather than lifted into the form that happens to own it.
	useEffect(() => {
		const target = p.targetRef.current;
		if (target === null || !enabled) return;

		// A drop only lands on a target that accepted the drag first; without
		// this the drop handler below never runs at all.
		const acceptDrag = (event: DragEvent) => {
			if (event.dataTransfer?.types.includes("Files") === true) event.preventDefault();
		};

		const onTransfer = (event: ClipboardEvent | DragEvent) => {
			const files = filesFromTransfer(
				"clipboardData" in event ? event.clipboardData : event.dataTransfer,
			);
			if (files.length === 0) return;
			// Only once there are files: otherwise this would swallow pasted text.
			event.preventDefault();
			setPending(files);
		};

		target.addEventListener("paste", onTransfer);
		target.addEventListener("drop", onTransfer);
		target.addEventListener("dragenter", acceptDrag);
		target.addEventListener("dragover", acceptDrag);
		return () => {
			target.removeEventListener("paste", onTransfer);
			target.removeEventListener("drop", onTransfer);
			target.removeEventListener("dragenter", acceptDrag);
			target.removeEventListener("dragover", acceptDrag);
		};
	}, [enabled, p.targetRef]);

	const confirm = () => {
		const files = pending;
		setPending([]);
		uploadFiles.mutate(files, {
			onSuccess: (uploads) => {
				const target = p.targetRef.current;
				if (target !== null)
					p.onInput(applyToTextarea(target, md.insert(uploadsToMarkdown(uploads))));
			},
		});
	};

	const reason = uploadFiles.isPending
		? "Uploading…"
		: signedIn
			? "Attach a file"
			: "Sign in to GitButler to attach files";

	return (
		<>
			<input
				accept={ACCEPTED_FILE_TYPES.join(",")}
				className={styles.picker}
				multiple
				onChange={(event) => {
					const files = Array.from(event.currentTarget.files ?? []);
					// Cleared so picking the same file twice in a row still fires.
					event.currentTarget.value = "";
					if (files.length > 0) setPending(files);
				}}
				ref={inputRef}
				tabIndex={-1}
				type="file"
			/>

			<Tooltip.Root>
				{/* Disabled buttons swallow hover, so the wrapper span carries the tooltip. */}
				<Tooltip.Trigger render={<span className={styles.triggerWrap} />}>
					<button
						aria-label="Attach a file"
						className={getButtonClassName({ variant: "ghost", iconOnly: true })}
						disabled={!enabled}
						onClick={() => inputRef.current?.click()}
						type="button"
					>
						<Icon name={uploadFiles.isPending ? "spinner" : "paperclip"} />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup />}>{reason}</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<AlertDialog.Root open={pending.length > 0} onOpenChange={(open) => open || setPending([])}>
				<AlertDialog.Portal>
					<AlertDialog.Backdrop />
					<AlertDialog.Popup
						className={classes(uiStyles.popup, uiStyles.dialogPopup, styles.popup)}
					>
						<AlertDialog.Title>
							{pending.length === 1 ? "Upload this file?" : `Upload these ${pending.length} files?`}
						</AlertDialog.Title>
						<AlertDialog.Description className={styles.description}>
							They are uploaded to gitbutler.com and anyone with the link can open them, which is
							what lets the forge show them in your description.
						</AlertDialog.Description>
						<ul className={styles.files}>
							{pending.map((file, index) => (
								// Names repeat — two pasted images are both "pasted-image" —
								// and the list is fixed while the dialog is open.
								// oxlint-disable-next-line react/no-array-index-key
								<li key={index}>{file.name}</li>
							))}
						</ul>
						<div className={styles.actions}>
							<button
								className={getButtonClassName({ variant: "ghost" })}
								onClick={() => setPending([])}
								type="button"
							>
								Cancel
							</button>
							<button
								className={getButtonClassName({ variant: "pop" })}
								onClick={confirm}
								type="button"
							>
								Yes, upload
							</button>
						</div>
					</AlertDialog.Popup>
				</AlertDialog.Portal>
			</AlertDialog.Root>
		</>
	);
};
