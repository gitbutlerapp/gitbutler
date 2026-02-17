import { buildLoadableTable } from "$lib/redux/defaultSlices";
import { type LoadableRule } from "$lib/rules/types";

export const rulesTable = buildLoadableTable<LoadableRule>("rules");
