import styles from "./DetailsPlaceholder.module.css";
import { EmptyState } from "#ui/components/EmptyState.tsx";
import type { FC, ReactNode } from "react";

/**
 * The details pane with nothing to detail.
 *
 * The pane only ever empties because the list driving it is empty, and that
 * list is already carrying its own empty state a few hundred pixels to the
 * left. So this says the one thing the sidebar cannot: what the pane itself is
 * for. Present tense — an instruction would be unfollowable, since there is
 * nothing in the list to act on.
 *
 * No actions, matching the component in ⚛️ Lite Core, which hides its actions
 * slot here: every action belonging to this state belongs to the section that
 * owns it, and would be out of context in the pane.
 */
export const DetailsPlaceholder: FC<{ title: string; description: ReactNode }> = ({
	title,
	description,
}) => (
	<div className={styles.host}>
		<EmptyState illustration="waving" title={title} description={description} />
	</div>
);
