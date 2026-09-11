import { ForgeLabel } from "#ui/components/ForgeLabel.tsx";
import type { ForgeReviewLabel } from "@gitbutler/but-sdk";
import { type FC, Fragment } from "react";
import { RowLabel, RowLabelContainer } from "./Row.tsx";
import styles from "./BranchRowHeadline.module.css";

export const BranchRowHeadline: FC<{
	title: string;
	labels?: ReadonlyArray<ForgeReviewLabel>;
}> = ({ title, labels }) => (
	<RowLabelContainer className={styles.headline}>
		<RowLabel heading className={styles.title} title={title}>
			{title}
		</RowLabel>
		{labels?.map((label) => (
			<Fragment key={label.name}>
				{" "}
				<ForgeLabel label={label} size="regular" />
			</Fragment>
		))}
	</RowLabelContainer>
);
