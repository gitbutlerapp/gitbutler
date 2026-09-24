import { useCreateReviewThreadReply } from "#ui/api/mutations.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { FieldTextareaStyles } from "@gitbutler/ui-react/Field.tsx";
import { useMentionSuggestions } from "#ui/components/MentionSuggestions.tsx";
import { type FC, type KeyboardEvent, useRef, useState } from "react";
import styles from "./ReviewThreadReply.module.css";

type Props = {
	projectId: string;
	/** The review the thread hangs on, which is how its cache is keyed. */
	reviewId: number;
	threadId: string;
};

/**
 * Reply to one diff comment thread, wherever the thread is shown. Folded to a
 * single button until asked for, so a file's worth of threads doesn't open as
 * a column of empty boxes.
 */
export const ReviewThreadReply: FC<Props> = ({ projectId, reviewId, threadId }) => {
	const [open, setOpen] = useState(false);
	const [body, setBody] = useState("");
	/** One-shot: the box takes focus when unfolded, not on every re-render. */
	const wantsFocusRef = useRef(false);
	const textareaRef = useRef<HTMLTextAreaElement | null>(null);
	const { mutate: reply } = useCreateReviewThreadReply(projectId, reviewId);
	const mentions = useMentionSuggestions({
		projectId,
		targetRef: textareaRef,
		value: body,
		onInput: setBody,
	});

	const submit = () => {
		const text = body.trim();
		if (text === "") return;
		// Optimistic: the reply is already in the thread, so the box closes —
		// and comes back with the text still in it if the forge refuses.
		setBody("");
		setOpen(false);
		reply(
			{ projectId, threadId, body: text },
			{
				onError: () => {
					setBody((current) => (current === "" ? text : current));
					setOpen(true);
				},
			},
		);
	};

	const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
		if (mentions.onKeyDown(event)) return;
		if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			submit();
			return;
		}
		if (event.key === "Escape") {
			event.preventDefault();
			setOpen(false);
		}
	};

	if (!open) {
		return (
			<Button
				variant="ghost"
				className={styles.open}
				onClick={() => {
					wantsFocusRef.current = true;
					setOpen(true);
				}}
			>
				Reply
			</Button>
		);
	}

	return (
		<div className={styles.composer}>
			<FieldTextareaStyles
				{...mentions.textareaProps}
				aria-label="Reply to this thread"
				className={styles.input}
				onKeyDown={onKeyDown}
				placeholder="Write a reply…"
				ref={(textarea) => {
					textareaRef.current = textarea;
					if (textarea && wantsFocusRef.current) {
						wantsFocusRef.current = false;
						textarea.focus();
					}
				}}
				rows={3}
				value={body}
			/>
			{mentions.popup}
			<div className={styles.actions}>
				<Button variant="ghost" onClick={() => setOpen(false)}>
					Cancel
				</Button>
				<Button variant="gray" disabled={body.trim() === ""} onClick={submit}>
					Reply
				</Button>
			</div>
		</div>
	);
};
