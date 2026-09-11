import { Badge } from "#ui/components/Badge.tsx";
import { branchReviewPresentation } from "#ui/branch.ts";
import type { ForgeReviewLabel } from "@gitbutler/but-sdk";
import { type FC, Fragment } from "react";
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
				{separator && "· "}
				{topics.map((label, index) => (
					<Fragment key={label.name}>
						{index > 0 && ", "}
						<span title={label.description ?? undefined}>{label.name}</span>
					</Fragment>
				))}
			</span>
		)
	);
};
