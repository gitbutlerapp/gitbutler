import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { operandEquals, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { selectProjectSelectionOutline } from "#ui/projects/state.ts";
import { resolveNavigationIndexSelection } from "#ui/selection-scopes.ts";
import { useAppSelector } from "#ui/store.ts";
import { assert } from "#ui/assert.ts";
import { use } from "react";

export const useIsSelected = ({
	projectId,
	operand,
}: {
	projectId: string;
	operand: Operand;
}): boolean => {
	const navigationIndex = assert(use(NavigationIndexContext));
	return useAppSelector((state) => {
		const selectionState = selectProjectSelectionOutline(state, projectId);
		const selection = resolveNavigationIndexSelection(
			navigationIndex,
			selectionState,
			operandIdentityKey,
		);

		return selection ? operandEquals(selection, operand) : false;
	});
};
