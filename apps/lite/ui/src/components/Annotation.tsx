import { classes } from "#ui/components/classes.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import type { FC, ReactNode, Ref } from "react";
import styles from "./Annotation.module.css";
import { FieldTextareaStyles } from "./Field.tsx";

type Props = {
	author: string;
	defaultBody: string;
	name: string;
	textareaRef?: Ref<HTMLTextAreaElement>;
	updatedAt?: number;
	actions?: ReactNode;
};

export const Annotation: FC<Props> = ({ textareaRef, ...p }) => (
	<div className={styles.wrapper}>
		<header className={styles.header}>
			<h5 className={classes(styles.author, "text-13")}>{p.author}</h5>
			{p.updatedAt !== undefined && (
				<time
					dateTime={new Date(p.updatedAt).toISOString()}
					className={classes(styles.date, "text-12")}
				>
					<RelativeTime timestamp={p.updatedAt} />
				</time>
			)}
		</header>

		<FieldTextareaStyles
			className={classes(styles.body, "text-13")}
			defaultValue={p.defaultBody}
			name={p.name}
			ref={textareaRef}
			required
			rows={3}
		/>

		<div className={styles.actions}>{p.actions}</div>
	</div>
);
