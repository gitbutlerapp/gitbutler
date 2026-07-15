import { createContext } from "react";

// TreeItem is the sole global selection subscriber for its subtree. Keeping the
// derived boolean local lets React Compiler preserve every unaffected row body.
export const TreeItemSelectionContext = createContext(false);
TreeItemSelectionContext.displayName = "TreeItemSelectionContext";
