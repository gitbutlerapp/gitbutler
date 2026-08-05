import { classes } from "#ui/components/classes.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import type { FC, ReactNode, Ref } from "react";
import styles from "./Annotation.module.css";
import { FieldTextareaStyles } from "./Field.tsx";

type Props = {
	author: string;
	defaultBody: string;
	formId: string;
	name: string;
	onBlur?: (body: string) => void;
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
			form={p.formId}
			name={p.name}
			ref={textareaRef}
			rows={3}
			onBlur={(event) => {
				const body = event.currentTarget.value;
				if (body !== p.defaultBody) p.onBlur?.(body);
			}}
		/>

		<div className={styles.actions}>{p.actions}</div>
	</div>
);
