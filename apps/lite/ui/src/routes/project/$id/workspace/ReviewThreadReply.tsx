import { useCreateReviewThreadReply } from "#ui/api/mutations.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldTextareaStyles } from "#ui/components/Field.tsx";
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
	const { mutate: reply } = useCreateReviewThreadReply(projectId, reviewId);

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
			<button
				className={classes(getButtonClassName({ variant: "ghost" }), styles.open)}
				onClick={() => {
					wantsFocusRef.current = true;
					setOpen(true);
				}}
				type="button"
			>
				Reply
			</button>
		);
	}

	return (
		<div className={styles.composer}>
			<FieldTextareaStyles
				aria-label="Reply to this thread"
				className={styles.input}
				onChange={(event) => setBody(event.currentTarget.value)}
				onKeyDown={onKeyDown}
				placeholder="Write a reply…"
				ref={(textarea) => {
					if (textarea && wantsFocusRef.current) {
						wantsFocusRef.current = false;
						textarea.focus();
					}
				}}
				rows={3}
				value={body}
			/>
			<div className={styles.actions}>
				<button
					className={getButtonClassName({ variant: "ghost" })}
					onClick={() => setOpen(false)}
					type="button"
				>
					Cancel
				</button>
				<button
					className={getButtonClassName({ variant: "gray" })}
					disabled={body.trim() === ""}
					onClick={submit}
					type="button"
				>
					Reply
				</button>
			</div>
		</div>
	);
};
