import styles from "./OperationTarget.module.css";
import type { Placement } from "#ui/operations/operation.ts";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { Match } from "effect";
import type { FC } from "react";

export type OperationTargetOutline = "inside" | "outside";

export const OperationTarget: FC<
	{
		placement: Placement | undefined;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ placement, outline, render, ...props }) =>
	useRender({
		render,
		props: mergeProps<"div">(props, {
			className: Match.value(placement).pipe(
				Match.when("above", () => classes(styles.insertionTarget, styles.insertionTargetAbove)),
				Match.when("below", () => classes(styles.insertionTarget, styles.insertionTargetBelow)),
				Match.when("into", () =>
					classes(
						styles.activeTarget,
						Match.value(outline).pipe(
							Match.when("inside", () => styles.activeTargetInside),
							Match.when("outside", () => styles.activeTargetOutside),
							Match.exhaustive,
						),
					),
				),
				Match.when(undefined, () => undefined),
				Match.exhaustive,
			),
		}),
	});
