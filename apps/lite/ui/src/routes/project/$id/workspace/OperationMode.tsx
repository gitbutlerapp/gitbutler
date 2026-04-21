import { moveOperation, rubOperation, type Operation } from "#ui/Operation.ts";
import { Match } from "effect";
import { itemEquals, type Item } from "./Item.ts";
import { type ResolvedOperationSource } from "./ResolvedOperationSource.ts";
import { type OperationMode } from "./WorkspaceMode.ts";

export const operationModeToOperation = ({
	operationMode,
	resolvedOperationSource,
	target,
}: {
	operationMode: OperationMode;
	resolvedOperationSource: ResolvedOperationSource;
	target: Item;
}): Operation | null =>
	Match.value(operationMode).pipe(
		Match.tagsExhaustive({
			Rub: () => rubOperation({ resolvedOperationSource, target }),
			Move: () => moveOperation({ resolvedOperationSource, target, side: "below" }),
		}),
	);

export const isOperationModeSourceOrTarget = ({
	item,
	operationMode,
	resolvedOperationSource,
}: {
	item: Item;
	operationMode: OperationMode;
	resolvedOperationSource: ResolvedOperationSource | null;
}): boolean =>
	itemEquals(operationMode.source, item) ||
	(!!resolvedOperationSource &&
		!!operationModeToOperation({
			operationMode,
			resolvedOperationSource,
			target: item,
		}));
