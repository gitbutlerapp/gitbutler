import type { Operand } from "#ui/operands.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { createContext } from "react";

export const NavigationIndexContext = createContext<NavigationIndex<Operand> | null>(null);
NavigationIndexContext.displayName = "NavigationIndexContext";
