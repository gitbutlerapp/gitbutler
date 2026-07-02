import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { Row } from "../Row.tsx";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { operandIdentityKey, type Operand } from "#ui/operands.ts";
import { navigationIndexIncludes } from "#ui/workspace/navigation-index.ts";
import { Tooltip } from "@base-ui/react";
import { ComponentProps, FC, use } from "react";
import { assert } from "#ui/assert.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useIsSelected } from "./useIsSelected.ts";
import styles from "./ItemRow.module.css";

const CommitTargetIndicator: FC = () => (
	<Tooltip.Root>
		<Tooltip.Trigger aria-label="Commit target" className={styles.commitTargetIndicator}>
			<svg
				aria-hidden
				xmlns="http://www.w3.org/2000/svg"
				width="20"
				height="13"
				viewBox="0 0 20 13"
				fill="none"
			>
				<path
					d="M11.7571 11.7906C10.5268 12.5802 9.09568 13 7.63376 13L6.5 13C2.91015 13 -1.65294e-06 10.0898 -1.3391e-06 6.5C-1.02527e-06 2.91015 2.91015 4.13306e-07 6.5 7.27141e-07L7.63377 8.26258e-07C9.09568 9.54062e-07 10.5268 0.419776 11.7571 1.20943L18.6888 5.65843C19.3019 6.05196 19.3019 6.94804 18.6888 7.34157L11.7571 11.7906Z"
					fill="#25B1B1"
				/>
				<circle cx="6.5" cy="6.5" r="3.75" stroke="var(--bg-1)" strokeWidth="1.5" />
				<circle cx="6.5" cy="6.5" r="0.75" stroke="var(--bg-1)" strokeWidth="1.5" />
			</svg>
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>Commit target</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);

export const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
		isCommitTarget?: boolean;
	} & Omit<ComponentProps<typeof Row>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, isCommitTarget, ...props }) => {
	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });
	const selectItem = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};

	return (
		<div className={styles.container}>
			<Row
				{...props}
				inert={!navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
				isSelected={isSelected}
				onSelect={selectItem}
			/>
			{isCommitTarget && <CommitTargetIndicator />}
		</div>
	);
};
