import { Badge } from "#ui/components/Badge.tsx";
import { ForgeLabel } from "#ui/components/ForgeLabel.tsx";
import { branchReviewPresentation } from "#ui/branch.ts";
import type { ForgeReviewLabel } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { RowLabel, RowLabelContainer } from "./Row.tsx";
import styles from "./BranchRowHeadline.module.css";

type ReviewProps = { title: string; labels?: ReadonlyArray<ForgeReviewLabel> };
export const BranchRowHeadline: FC<ReviewProps> = ({ title, labels }) => {
	const presentation = branchReviewPresentation(title, labels);
	return (
		<RowLabelContainer className={styles.headline}>
			<RowLabel heading className={styles.title} title={presentation.title}>
				{presentation.title}
			</RowLabel>
		</RowLabelContainer>
	);
};

export const BranchReviewTag: FC<ReviewProps> = ({ title, labels }) => {
	const { action } = branchReviewPresentation(title, labels);
	return (
		action !== undefined && (
			<div className={styles.actionLine}>
				<Badge
					variant={action === "Screenshots needed" ? "warn" : "fillGray"}
					className={action === "Screenshots needed" ? styles.screenshots : undefined}
				>
					{action}
				</Badge>
			</div>
		)
	);
};

export const BranchTopics: FC<{
	labels?: ReadonlyArray<ForgeReviewLabel>;
	separator?: boolean;
}> = ({ labels, separator = false }) => {
	const { topics } = branchReviewPresentation("", labels);
	return (
		topics.length > 0 && (
			<span className={styles.topics}>
				{separator && <span aria-hidden="true">·</span>}
				{topics.map((label) => (
					<ForgeLabel key={label.name} label={label} size="regular" />
				))}
			</span>
		)
	);
};
