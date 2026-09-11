import styles from "./DetailsPlaceholder.module.css";
import { EmptyState } from "#ui/components/EmptyState.tsx";
import type { FC, ReactNode } from "react";

export const DetailsPlaceholder: FC<{ title: string; description: ReactNode }> = ({
	title,
	description,
}) => (
	<div className={styles.host}>
		<EmptyState illustration="waving" title={title} description={description} />
	</div>
);
