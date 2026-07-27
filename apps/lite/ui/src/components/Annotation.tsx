import { classes } from "#ui/components/classes.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { FC, ReactNode } from "react";
import styles from "./Annotation.module.css";
import { FieldTextareaStyles } from "./Field.tsx";

type Props = {
	author: string;
	defaultBody: string;
	onBlur?: (body: string) => void;
	updatedAt?: number;
	actions?: ReactNode;
};

export const Annotation: FC<Props> = (p) => (
	<div className={styles.wrapper}>
		<header className={styles.header}>
			<h5 className={classes(styles.author, "text-13")}>{p.author}</h5>
			{p.updatedAt !== undefined && (
				<time
					dateTime={new Date(p.updatedAt).toISOString()}
					className={classes(styles.date, "text-12")}
				>
					{formatRelativeTime(p.updatedAt)}
				</time>
			)}
		</header>

		<FieldTextareaStyles
			className={classes(styles.body, "text-13")}
			defaultValue={p.defaultBody}
			rows={3}
			onBlur={(event) => {
				const body = event.currentTarget.value;
				if (body !== p.defaultBody) p.onBlur?.(body);
			}}
			// oxlint-disable-next-line jsx-a11y/no-autofocus -- Focus shifts in response to user action.
			autoFocus
		/>

		<div className={styles.actions}>{p.actions}</div>
	</div>
);
