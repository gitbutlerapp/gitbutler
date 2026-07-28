import { WorkspaceApp } from "./WorkspaceApp.tsx";
import { createRoot } from "react-dom/client";
import "./styles.css";

const root = document.getElementById("root");

if (root === null) {
	throw new Error("GitButler MCP App root element is missing");
}

createRoot(root).render(<WorkspaceApp />);
