import type { Address } from "#ui/addresses.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { createContext } from "react";

export const NavigationIndexContext = createContext<NavigationIndex<Address> | null>(null);
NavigationIndexContext.displayName = "NavigationIndexContext";
